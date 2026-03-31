fn main() {
    match benchmarks::run_from_args(std::env::args().skip(1)) {
        Ok((report, benchmarks::OutputFormat::Json)) => {
            println!("{}", report.to_json());
        }
        Ok((report, benchmarks::OutputFormat::Table)) => {
            println!("{}", report.to_table());
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
