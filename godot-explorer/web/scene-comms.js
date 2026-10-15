(() => {
    const MAX_PACKET = 64 * 1024;
    const MAX_BYTES = 4 * 1024 * 1024;
    const MAX_MESSAGES = 500;

    globalThis.DclSceneComms = class {
        constructor(requestAdapter, onPacket, onState) {
            this.requestAdapter = requestAdapter;
            this.onPacket = onPacket;
            this.onState = onState;
            this.queue = [];
            this.bytes = 0;
            this.closed = false;
            this.ready = false;
            this.sending = false;
            this.attempt = 0;
            this.connect();
        }

        async connect() {
            try {
                const adapter = await this.requestAdapter();
                if (this.closed) return;
                if (!adapter.startsWith('livekit:')) throw new Error('Unsupported scene adapter');
                const url = new URL(adapter.slice('livekit:'.length));
                const token = url.searchParams.get('access_token');
                if (url.protocol !== 'wss:' || !token) throw new Error('Invalid scene adapter');
                url.searchParams.delete('access_token');
                const room = new LivekitClient.Room({ disconnectOnPageLeave: true });
                this.room = room;
                room.on(LivekitClient.RoomEvent.DataReceived, (packet, participant) => {
                    const sender = participant?.identity;
                    if (this.closed || packet.byteLength > MAX_PACKET ||
                        (sender !== 'authoritative-server' && !/^0x[0-9a-f]{40}$/i.test(sender))) return;
                    this.onPacket(sender.toLowerCase(), packet);
                });
                room.on(LivekitClient.RoomEvent.Reconnecting, () => this.setReady(false));
                room.on(LivekitClient.RoomEvent.Reconnected, () => { this.setReady(true); this.flush(); });
                room.on(LivekitClient.RoomEvent.Disconnected, () => this.retry());
                await room.connect(url.toString(), token, { autoSubscribe: false });
                if (this.closed || this.room !== room) { await room.disconnect(); return; }
                this.setReady(true);
                this.attempt = 0;
                console.info('[scene-comms] connected');
                this.flush();
            } catch {
                if (!this.closed) {
                    console.warn('[scene-comms] connection failed; retrying');
                    this.retry();
                }
            }
        }

        retry() {
            this.setReady(false);
            if (this.closed || this.timer) return;
            this.room?.removeAllListeners();
            void this.room?.disconnect();
            this.room = null;
            this.timer = setTimeout(() => {
                this.timer = null;
                this.connect();
            }, Math.min(1000 * 2 ** this.attempt++, 30000));
        }

        setReady(ready) {
            this.ready = ready;
            this.onState(ready);
        }

        send(packet, recipient) {
            if (this.closed || packet.byteLength > MAX_PACKET || this.queue.length >= MAX_MESSAGES ||
                this.bytes + packet.byteLength > MAX_BYTES) return false;
            if (recipient && recipient !== 'authoritative-server' && !/^0x[0-9a-f]{40}$/i.test(recipient)) return false;
            this.queue.push({ packet, recipient: recipient.toLowerCase() });
            this.bytes += packet.byteLength;
            // A synchronous publish failure must not re-enter Rust's scene-state borrow.
            queueMicrotask(() => this.flush());
            return true;
        }

        async flush() {
            if (this.sending) return;
            const room = this.room;
            this.sending = true;
            try {
                while (!this.closed && this.ready && this.room === room && this.queue.length) {
                    const { packet, recipient } = this.queue.shift();
                    this.bytes -= packet.byteLength;
                    await room.localParticipant.publishData(packet, {
                        reliable: true,
                        destinationIdentities: recipient ? [recipient] : []
                    });
                }
            } catch {
                console.warn('[scene-comms] send failed; reconnecting');
                if (this.room === room) this.retry();
            } finally {
                this.sending = false;
                if (!this.closed && this.ready && this.queue.length) this.flush();
            }
        }

        close() {
            this.closed = true;
            this.setReady(false);
            clearTimeout(this.timer);
            this.queue.length = 0;
            this.bytes = 0;
            this.room?.removeAllListeners();
            void this.room?.disconnect();
            this.room = null;
        }
    };
})();
