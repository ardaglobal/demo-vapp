use std::io::{self, Write};

pub struct ArithmeticSession {
    pub a: i32,
    pub b: i32,
}

impl ArithmeticSession {
    pub fn to_circuit_inputs(&self) -> (i32, i32, i32) {
        let result = arithmetic_lib::addition(self.a, self.b);
        (self.a, self.b, result)
    }
}

pub fn get_arithmetic_inputs() -> Option<ArithmeticSession> {
    println!("🧮 Arithmetic ZK Proof Generator");
    println!("=================================");
    println!("This will generate a zero-knowledge proof that you correctly");
    println!("computed the addition of two numbers without revealing the numbers themselves.\n");

    let a = get_number_input("Enter the first number (a): ")?;
    let b = get_number_input("Enter the second number (b): ")?;
    
    let result = arithmetic_lib::addition(a, b);
    
    println!("\n📊 Computation Summary:");
    println!("   • First number (a): {}", a);
    println!("   • Second number (b): {}", b);
    println!("   • Result (a + b): {}", result);
    println!("\n🔒 Generating ZK proof that this computation is correct...\n");

    Some(ArithmeticSession { a, b })
}

fn get_number_input(prompt: &str) -> Option<i32> {
    loop {
        print!("{}", prompt);
        io::stdout().flush().ok()?;
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                match input.trim().parse::<i32>() {
                    Ok(num) => return Some(num),
                    Err(_) => {
                        println!("❌ Invalid input. Please enter a valid integer.");
                        continue;
                    }
                }
            }
            Err(_) => {
                println!("❌ Error reading input.");
                return None;
            }
        }
    }
}