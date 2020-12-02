use std::fmt::Display;
use std::ops::Range;
use std::process::exit;

pub trait VecExt<T> {
    fn push_and_check(&mut self, to_push: T);
}

impl<T: std::fmt::Debug> VecExt<T> for Vec<T> {
    #[cfg(debug_assertions)]
    #[inline]
    fn push_and_check(&mut self, to_push: T) {
        if self.len() >= self.capacity() {
            panic!("Exceeded capacity {:?}", self);
        } else {
            self.push(to_push);
        }
    }
    #[cfg(not(debug_assertions))]
    #[inline]
    fn push_and_check(&mut self, to_push: T) {
        self.push(to_push);
    }
}

// clone of 'then_some()' in nightly: bool_to_option #64260
pub trait BoolExt {
    fn to_some<T>(self, item: T) -> Option<T>;
    //fn or_die(self, msg: String);
}
impl BoolExt for bool {
    #[inline]
    fn to_some<T>(self, item: T) -> Option<T> {
        if self {
            Some(item)
        } else {
            None
        }
    }

    //#[inline]
    //fn or_die(self, msg: String) {
    //    if !self {
    //        eprintln!("{}", msg);
    //        exit(1)
    //    }
    //}
}

// Escape single qoutes and add surrounding single quotes
pub trait ShellEscape: AsRef<str> {
    fn escape(&self) -> String {
        let substr = self.as_ref();
        let escapees = substr.chars().filter(|c| *c == '\'').count();
        let capacity = substr.len()
            + escapees * "'\\''".len() // times four per single-quote in substr
            + '\''.len_utf8() * 2      // leading and trailing single quotes
        ;
        let mut output = String::with_capacity(capacity);
        output.push('\'');
        for c in substr.chars() {
            if c == '\'' {
                output.push_str("'\\''");
            } else {
                output.push(c);
            }
        }
        output.push('\'');
        output
    }
}
impl ShellEscape for str {}
impl ShellEscape for String {}


// 
pub trait ResultExt<T, E: Display> {
    fn or_die(self, exit_code: i32) -> T;
}
impl<T, E: Display> ResultExt<T, E> for Result<T, E> {
    fn or_die(self, exit_code: i32) -> T {
        match self {
            Ok(x) => x,
            Err(err) => {
                eprintln!("{}", err);
                exit(exit_code)
            }
        }
    }
}

// This was used for Post and Frontmatter parsing; now only used for the latter
// Although I could implement a more specialised iterator for Frontmatter,
// I am keeping this here as a reference for other, future projects
pub type RStr = Range<usize>;

//trait RangeExt: std::slice::SliceIndex<str> {
pub trait RangeExt {
    fn of<'a>(&self, original: &'a str) -> &'a str;
    fn expand(&mut self, till: usize) -> &mut Self;
    fn split<'a>(&self, original: &'a str) -> RangeSplitInclusive<'a>;
    fn split_over<'a>(
        &self,
        original: &'a str,
        delimiter: fn(&str) -> RStr,
    ) -> RangeSplitInclusive<'a>;
}

impl RangeExt for RStr {
    fn of<'a>(&self, original: &'a str) -> &'a str {
        &original[self.start..self.end]
    }
    fn expand(&mut self, till: usize) -> &mut Self {
        self.end = till;
        self
    }
    fn split<'a>(&self, original: &'a str) -> RangeSplitInclusive<'a> {
        RangeSplitInclusive {
            buffer: &original[self.start..self.end],
            delimit_by: |substr| {
                let len = substr.len();
                substr.find('\n').map(|i| i + 1..i + 1).unwrap_or(len..len)
            },
            index: self.start,
        }
    }
    fn split_over<'a>(
        &self,
        original: &'a str,
        delimit_by: fn(&str) -> RStr,
    ) -> RangeSplitInclusive<'a> {
        RangeSplitInclusive {
            buffer: &original[self.start..self.end],
            delimit_by,
            index: self.start,
        }
    }
}

pub struct RangeSplitInclusive<'a> {
    buffer: &'a str,
    delimit_by: fn(&str) -> RStr,
    index: usize,
}

impl<'a> Iterator for RangeSplitInclusive<'a> {
    type Item = (RStr, RStr);
    fn next(&mut self) -> Option<Self::Item> {
        let rel_delim = (self.delimit_by)(self.buffer);
        let buffer_len = self.buffer.len();
        //self.a.chars().fold(0, |mut acc, a| {
        //    println!("({}, {:?})", acc, a);
        //    acc += a.len_utf8();
        //    acc
        //});
        if buffer_len > 0 {
            let start = self.index;
            self.index += rel_delim.end;
            self.buffer = &self.buffer[rel_delim.end..];
            let delimiter = rel_delim.start + start..rel_delim.end + start;
            Some((start..start + rel_delim.start, delimiter))
        } else {
            None
        }
    }
}

#[test]
fn split_test() {
    let body = "hello你\n你how\n are you tody 你好嗎";
    (2..body.len() - 6).split(body).for_each(|a| {
        assert!(a.1.of(body).is_empty());
        println!("{:?} {:?}", a, a.0.of(body));
    });
    println!("===");

    (1..body.len() - 6)
        .split_over(body, |substr| {
            substr
                .find("\n")
                .map(|i| i..i + 1)
                .unwrap_or(substr.len()..substr.len())
        })
        .for_each(|a| {
            println!("{:?} {:?} {:?}", a, a.0.of(body), a.1.of(body));
        });
}
