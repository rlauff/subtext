// Runs the Subtext wasm interpreter off the main thread, so the page stays
// responsive during long evaluations and "Stop" can terminate a run.
//
// The wasm bindings expect `window.subtextPrint`; inside a worker there is no
// `window`, so we alias it to the worker global before loading the module.
self.window = self;
self.subtextPrint = function (text) {
    self.postMessage({ type: "print", text: String(text) });
};

let run_wasm = null;
let wasm_mod = null;

self.onmessage = async function (e) {
    const msg = e.data;

    if (msg.type === "init") {
        try {
            const mod = await import("./pkg/subtext.js");
            await mod.default();
            wasm_mod = mod;
            run_wasm = mod.run_wasm;
            self.postMessage({ type: "ready" });
        } catch (err) {
            self.postMessage({ type: "init-error", text: String(err) });
        }
        return;
    }

    // TODO(review): new — long-form error explanations for the terminal's explain button.
    if (msg.type === "explain") {
        if (wasm_mod && typeof wasm_mod.explain_wasm === "function") {
            self.postMessage({
                type: "print",
                text: "\u2500\u2500 explain[" + msg.code + "] \u2500\u2500\n"
                    + wasm_mod.explain_wasm(msg.code)
                    + "\n\u2500\u2500 end explain \u2500\u2500",
            });
        } else {
            self.postMessage({ type: "print", text: "note: explanations require a rebuilt pkg (wasm-pack build --target web)" });
        }
        return;
    }

    if (msg.type === "run") {
        if (!run_wasm) {
            self.postMessage({ type: "error", text: "wasm module is not loaded" });
            return;
        }
        const t0 = performance.now();
        try {
            run_wasm(msg.code);
            self.postMessage({ type: "done", ms: performance.now() - t0 });
        } catch (err) {
            self.postMessage({ type: "error", text: String(err) });
        }
    }
};
