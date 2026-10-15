// fetch() bridge for dcl-web-reqwest. Strings are passed as (ptr, len) and decoded
// with TextDecoder, so nothing here depends on optional Emscripten runtime helpers.
#include <emscripten.h>
#include <stdint.h>

typedef void (*dcl_fetch_done_cb)(int id, int status, int body_len, int meta_len);

EM_JS(void, dcl_fetch_start,
      (int id, const char *method, int method_len, const char *url, int url_len,
       const char *headers_json, int headers_len, const uint8_t *body, int body_len,
       int timeout_ms, int follow_redirects, dcl_fetch_done_cb done),
      {
          const dec = new TextDecoder();
          const str = (p, n) => dec.decode(HEAPU8.slice(p, p + n));
          const store = (Module.__dclFetch = Module.__dclFetch || new Map());
          const headers = new Headers();
          for (const [k, v] of JSON.parse(str(headers_json, headers_len))) {
              try { headers.append(k, v); } catch (e) { /* forbidden header (User-Agent, ...) */ }
          }
          const ctrl = new AbortController();
          const init = { method: str(method, method_len), headers, signal: ctrl.signal,
                         redirect: follow_redirects ? 'follow' : 'manual' };
          if (body_len > 0) init.body = HEAPU8.slice(body, body + body_len);
          const timer = timeout_ms > 0 ? setTimeout(() => ctrl.abort(), timeout_ms) : 0;
          const finish = (status, bytes, meta) => {
              if (timer) clearTimeout(timer);
              const metaBytes = new TextEncoder().encode(meta);
              store.set(id, { body: bytes, meta: metaBytes });
              wasmTable.get(done)(id, status, bytes.length, metaBytes.length);
          };
          fetch(str(url, url_len), init)
              .then(r => r.arrayBuffer().then(ab =>
                  finish(r.status, new Uint8Array(ab), JSON.stringify([r.url, [...r.headers.entries()]]))))
              .catch(e => finish(ctrl.signal.aborted ? -1 : 0, new Uint8Array(0), String((e && e.message) || e)));
      });

EM_JS(void, dcl_fetch_take, (int id, uint8_t *body_dst, uint8_t *meta_dst), {
    const store = Module.__dclFetch;
    const entry = store && store.get(id);
    if (!entry) return;
    store.delete(id);
    if (body_dst) HEAPU8.set(entry.body, body_dst);
    if (meta_dst) HEAPU8.set(entry.meta, meta_dst);
});
