use sp1_build::{build_program_with_args, BuildArgs};

fn main() {
    let args = BuildArgs {
        output_directory: Some("../build".to_string()),
        ..Default::default()
    };
    build_program_with_args("../program", args);
}
