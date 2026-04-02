use js_sys::Function;
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = subtextPrint)]
    pub fn js_print(s: &str);
}

#[macro_export]
macro_rules! subtext_println {
    ($($arg:tt)*) => {{
        let formatted = format!($($arg)*);

        #[cfg(not(target_arch = "wasm32"))]
        {
            println!("{}", formatted);
        }

        #[cfg(target_arch = "wasm32")]
        {
            $crate::js_print(&formatted);
        }
    }};
}

pub mod error;
pub mod interpreter;
pub mod linked_chars;
pub mod scope;

use interpreter::Interpreter;
use linked_chars::LinkedChars;

pub fn run_code_logic(input_string: String) -> Result<(), error::SubtextError> {
    let mut root_interpreter = Interpreter {
        state: LinkedChars::from_iter(input_string.chars()),
        registers: vec![],
        functions: vec![],
        parent: None,
        history: None,
    };

    root_interpreter.evaluate()
}

#[wasm_bindgen]
pub fn run_wasm(code: &str, print_callback: &Function) {
    let print = |text: &str| {
        let this = JsValue::null();
        let message = JsValue::from_str(text);
        let _ = print_callback.call1(&this, &message);
    };

    match run_code_logic(code.to_string()) {
        Ok(_) => {}
        Err(err) => {
            subtext_println!(&format!("Error: {}", err));
        }
    }
}
