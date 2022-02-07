use std::thread_local;

use std::fmt::Write;

#[derive(Default)]
pub struct OutputCapture {
    #[cfg(test)]
    output: String,
}

impl OutputCapture {
    // pub fn new() -> Self {
    //     Self {
    //         #[cfg(test)]
    //         output: String::new()
    //     }
    // }

    #[cfg(test)]
    fn get_output(&self) -> String {
        self.output.clone()
    }

    #[cfg(test)]
    fn clear_output(&mut self) {
        self.output = String::new();
    }
}

impl std::fmt::Write for OutputCapture {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        #[cfg(test)]
        {
            self.output += s;
        }

        print!("{s}");
        Ok(())
    }
}

thread_local! {
    static STDOUT: std::cell::RefCell<OutputCapture> = Default::default();
}

pub fn output_string<S: AsRef<str>>(s: S) {
    STDOUT.with(|writer| {
        write!(writer.borrow_mut(), "{}", s.as_ref()).unwrap();
    });
}

#[cfg(test)]
pub fn get_output() -> String {
    STDOUT.with(|output_capture| output_capture.borrow().get_output())
}

#[cfg(test)]
pub fn clear_output() {
    STDOUT.with(|output_capture| output_capture.borrow_mut().clear_output())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_capture_works() {
        clear_output();
        output_string("foobar");
        assert_eq!(get_output(), String::from("foobar"));

        clear_output();
        assert_eq!(get_output(), String::from(""));
    }
}
