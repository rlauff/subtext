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

    root_interpreter.evaluate().map(|_| ())
}

#[wasm_bindgen]
pub fn run_wasm(code: &str) {
    // Führe die Interpreter-Logik mit dem übergebenen Code aus
    match run_code_logic(code.to_string()) {
        Ok(_) => {} // Alles lief fehlerfrei durch, keine weitere Aktion nötig
        Err(err) => {
            // Nutze dein Makro, das auch schon bei print_output() reibungslos funktioniert!
            subtext_println!("Error: {}", err);
        }
    }
}
