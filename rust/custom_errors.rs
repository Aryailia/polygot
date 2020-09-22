use std::borrow::Cow;
use std::fmt;

fn pad(buffer: &mut String, len: usize, to_pad: &str) {
    assert!(to_pad.len() <= len);
    buffer.push_str(to_pad);
    for _ in 0..len - to_pad.len() {
        buffer.push(' ');
    }
}

//run: cargo test custom_errors -- --nocapture
#[test]
fn error_writer_allocations() {
    let err: ParseError = (1, "", Cow::from("the cat")).into();
    assert!(!err.to_string().is_empty());
    let err: ParseError = (98, "\n\n", Cow::from("the cat")).into();
    assert!(!err.to_string().is_empty());
    let err: ParseError = (10, "The caturday\n", Cow::from("the mat")).into();
    assert!(!err.to_string().is_empty());
    assert!(!err.with_filename("fat.adoc").to_string().is_empty());
}

//#[derive(Debug)]
//pub enum ParseErrorKind {
//    Fatal,
//    Warning,
//}

#[derive(Debug)]
pub struct ParseError<'a> {
    //kind: ParseErrorKind,
    row: usize,
    context: &'a str,
    message: Cow<'a, str>,
}

impl<'a> ParseError<'a> {
    pub fn with_filename<'b>(self, filename: &'b str) -> FullParseError<'a, 'b> {
        FullParseError {
            filename,
            error: self,
        }
    }

    //#[inline]
    //pub fn warn(row: usize, context: &'a str, message: Cow<'a, str>) {
    //    //Self {
    //    //    kind: ParseErrorKind::Warning,
    //    //    row: error.0,
    //    //    context: error.1,
    //    //    message: error.2,
    //    //}
    //}

    fn line_count(&self) -> (bool, usize) {
        let has_trailing_newline = self
            .context
            .chars()
            .last()
            .map(|c| c == '\n')
            .unwrap_or(false);
        let line_count = self.context.lines().count()
            + if has_trailing_newline { 1 } else { 0 }
            + if self.context.is_empty() { 1 } else { 0 };
        (has_trailing_newline, line_count)
    }
}

#[derive(Debug)]
pub struct FullParseError<'a, 'b> {
    filename: &'b str,
    error: ParseError<'a>,
}
impl<'a, 'b> fmt::Display for FullParseError<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (_, line_count) = self.error.line_count();
        debug_assert!(self.error.row > 0);
        let last_row_string = (self.error.row + line_count - 1).to_string();
        let digit_len = last_row_string.len();
        let row_string = self.error.row.to_string();

        let capacity = digit_len + 4    // + 4 for "--> "
            + self.filename.len()
            + 1 + row_string.len()      // + 1 for colon
            + 1 + last_row_string.len() // + 1 for colon
            + 1                         // newline
            ;
        let mut buffer = String::with_capacity(capacity);
        pad(&mut buffer, digit_len, "");
        buffer.push_str("--> ");
        buffer.push_str(self.filename);
        buffer.push(':');
        buffer.push_str(row_string.as_str());
        buffer.push(':');
        buffer.push_str(last_row_string.as_str());
        buffer.push('\n');

        debug_assert_eq!(buffer.len(), capacity);
        f.write_str(buffer.as_str())?;
        f.write_str(&self.error.to_string())
    }
}

type Info<'a> = (usize, &'a str, Cow<'a, str>);
impl<'a> From<Info<'a>> for ParseError<'a> {
    fn from(tuple: Info<'a>) -> Self {
        assert!(tuple.0 > 0);
        Self {
            //kind: ParseErrorKind::Fatal,
            row: tuple.0,
            context: tuple.1,
            message: tuple.2,
        }
    }
}

// Colourless copy of rust's error compiler error format
impl<'a> fmt::Display for ParseError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        assert!(self.row > 0);

        let (has_trailing_newline, line_count) = self.line_count();
        let last_row_string = (self.row + line_count - 1).to_string();
        let digit_len = last_row_string.len();
        //println!("{:?}", line_count);

        let capacity = (digit_len + 3) * line_count
                                     // (digit_len + 3) for " | "
            + self.context.len() + 1 // + 1 for trailing \n, other \n's included
            + (digit_len + 3) * 3    // * 3 forstarting + trading + message
            + self.message.len();
        let mut buffer = String::with_capacity(capacity);

        // Vertical padding preceeding the context
        pad(&mut buffer, digit_len, "");
        buffer.push_str(" |\n");

        // Printing the context padded with line numbers
        // Do-while loop because "".lines() returns None immediately
        let mut iter = self.context.lines();
        let mut line = iter.next().unwrap_or("");
        let mut i = 0;
        loop {
            let row = self.row + i;
            pad(&mut buffer, digit_len, &row.to_string());
            buffer.push_str(" | ");
            buffer.push_str(line);
            buffer.push('\n');
            if let Some(next) = iter.next() {
                line = next;
                i += 1;
            } else {
                break;
            }
        }
        if has_trailing_newline {
            pad(&mut buffer, digit_len, last_row_string.as_str());
            buffer.push_str(" | ");
            buffer.push('\n');
        }

        // Vertical padding trailing the context
        pad(&mut buffer, digit_len, "");
        buffer.push_str(" |\n");

        // The message
        pad(&mut buffer, digit_len, "");
        buffer.push_str(" = ");
        buffer.push_str(&self.message);

        //println!("{} {}\n", buffer.len(), capacity);
        debug_assert_eq!(buffer.len(), capacity);
        f.write_str(buffer.as_str())
    }
}
