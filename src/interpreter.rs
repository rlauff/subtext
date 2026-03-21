use crate::linkes_chars::LinkedChars;

enum Task {
    Scope {
        content: String,
    },
    FunctionCall {
        function_name: String,
        input: String,
    },
    RegisterCall {
        level: usize,
        index: usize,
    },
    GetInput {
        prompt: String,
    },
    PrintOutput {
        content: String,
    },
}

struct Job {
    start: usize, // the ster index of the stuff to be replaced
    end: usize,   // end index
    task: Task,
}

fn get_new_job(linked_chars: &LinkedChars, reader_idx: usize) -> Job {
    let mut chars_buffer = Vec::new(); // holds the read chars
    loop {
        match linked_chars.get(reader_idx).c {
unimplemented!()
        }
    }
}
