(() => {
    const MAX_MESSAGE = 4096;
    const MAX_QUEUED_BYTES = 256 * 1024;

    globalThis.DclPulseTransport = class {
        constructor(url, emit) {
            this.emit = emit;
            this.closed = false;
            this.ready = false;
            this.reliable = [];
            this.queuedBytes = 0;
            this.datagram = null;
            this.sequences = [0, 0, 0];
            this.readers = [];
            this.writers = [];
            this.pageHide = () => this.finish(3, 1);
            globalThis.addEventListener?.('pagehide', this.pageHide);
            queueMicrotask(() => this.connect(url));
        }

        async connect(url) {
            if (this.closed) return;
            try {
                const endpoint = new URL(url);
                if (endpoint.protocol !== 'https:' || endpoint.username || endpoint.password || endpoint.search || endpoint.hash) {
                    throw new Error('Invalid Pulse endpoint');
                }
                const transport = this.transport = new WebTransport(endpoint.href);
                transport.closed.then(info => this.finish(3, info.closeCode || 0), () => this.finish(this.ready ? 3 : 4, 0));
                this.timeout = setTimeout(() => this.finish(4, 0), 20000);
                await transport.ready;
                if (this.closed) return;
                const stream = await transport.createBidirectionalStream();
                if (this.closed) return;
                this.streamWriter = stream.writable.getWriter();
                this.datagramWriter = transport.datagrams.writable.getWriter();
                this.writers.push(this.streamWriter, this.datagramWriter);
                this.readers.push(stream.readable.getReader(), transport.datagrams.readable.getReader());
                clearTimeout(this.timeout);
                this.ready = true;
                this.emit(0, 0);
                void this.readStream(this.readers[0]);
                void this.readDatagrams(this.readers[1]);
            } catch {
                this.finish(4, 0);
            }
        }

        async readStream(reader) {
            // Pulse streams use a big-endian u32 length; inbound datagrams are bare protobuf.
            const header = new Uint8Array(4);
            let headerBytes = 0, payload = null, used = 0;
            try {
                while (!this.closed) {
                    const { value, done } = await reader.read();
                    if (this.closed) return;
                    if (done) throw new Error('Pulse stream ended');
                    let offset = 0;
                    while (offset < value.length) {
                        if (headerBytes < 4) {
                            const count = Math.min(4 - headerBytes, value.length - offset);
                            header.set(value.subarray(offset, offset + count), headerBytes);
                            headerBytes += count;
                            offset += count;
                            if (headerBytes < 4) continue;
                            const length = new DataView(header.buffer).getUint32(0);
                            if (length > MAX_MESSAGE) throw new Error('Oversized Pulse message');
                            payload = new Uint8Array(length);
                            used = 0;
                        }
                        const count = Math.min(payload.length - used, value.length - offset);
                        payload.set(value.subarray(offset, offset + count), used);
                        offset += count;
                        used += count;
                        if (used === payload.length) {
                            if (!this.emit(1, payload)) throw new Error('Pulse receive queue full');
                            headerBytes = 0;
                        }
                    }
                }
            } catch {
                this.finish(3, 0);
            }
        }

        async readDatagrams(reader) {
            try {
                while (!this.closed) {
                    const { value, done } = await reader.read();
                    if (this.closed) return;
                    if (done) throw new Error('Pulse datagrams ended');
                    if (value.length <= MAX_MESSAGE) this.emit(2, value);
                }
            } catch {
                this.finish(3, 0);
            }
        }

        send(channel, payload) {
            if (this.closed || !this.ready || payload.length > MAX_MESSAGE || ![0, 1, 2].includes(channel)) return false;
            if (channel === 0) {
                if (this.reliable.length >= 256 || this.queuedBytes + payload.length > MAX_QUEUED_BYTES) return false;
                const frame = new Uint8Array(4 + payload.length);
                new DataView(frame.buffer).setUint32(0, payload.length);
                frame.set(payload, 4);
                this.reliable.push(frame);
                this.queuedBytes += payload.length;
                queueMicrotask(() => this.flushReliable());
            } else {
                const frame = new Uint8Array(5 + payload.length);
                if (frame.length > this.transport.datagrams.maxDatagramSize) return true;
                frame[0] = channel;
                this.sequences[channel] = (this.sequences[channel] + 1) >>> 0;
                new DataView(frame.buffer).setUint32(1, this.sequences[channel]);
                frame.set(payload, 5);
                // Keep the freshest movement while the browser applies backpressure.
                this.datagram = frame;
                queueMicrotask(() => this.flushDatagram());
            }
            return true;
        }

        async flushReliable() {
            if (this.writingReliable) return;
            this.writingReliable = true;
            try {
                while (!this.closed && this.reliable.length) {
                    const frame = this.reliable.shift();
                    this.queuedBytes -= frame.length - 4;
                    await this.streamWriter.write(frame);
                }
            } catch {
                this.finish(3, 0);
            } finally {
                this.writingReliable = false;
            }
        }

        async flushDatagram() {
            if (this.writingDatagram) return;
            this.writingDatagram = true;
            try {
                while (!this.closed && this.datagram) {
                    const frame = this.datagram;
                    this.datagram = null;
                    await this.datagramWriter.write(frame);
                }
            } catch {
                // Datagrams may be dropped without losing the reliable session.
                this.datagram = null;
            } finally {
                this.writingDatagram = false;
            }
        }

        finish(kind, code) {
            if (this.closed) return;
            this.close();
            this.emit(kind, code);
        }

        close() {
            if (this.closed) return;
            this.closed = true;
            this.ready = false;
            clearTimeout(this.timeout);
            globalThis.removeEventListener?.('pagehide', this.pageHide);
            this.reliable.length = 0;
            this.queuedBytes = 0;
            this.datagram = null;
            this.transport?.close({ closeCode: 1 });
            for (const reader of this.readers) void reader.cancel().catch(() => {});
            for (const writer of this.writers) void writer.abort().catch(() => {});
            this.readers.length = 0;
            this.writers.length = 0;
        }
    };
})();
