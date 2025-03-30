use std::io::{self, Write};

fn main() {
    let prompt = "$ ";

    loop {
        print!("{prompt}");
        io::stdout().flush().unwrap();

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let output = evaluate(input.trim());
        println!("{output}");
    }
}

fn evaluate(cmd: &str) -> String {
    format!("{cmd}: command not found")
}
