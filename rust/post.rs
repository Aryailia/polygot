type RStr = Range<usize>;
type WipPart<'a> = (Option<Vec<&'a str>>, RStr);
const API_SET_LANGUAGE: &'static str = "api_set_lang:";
const ALL_LANG: Option<&str> = None;

// https://cfsamson.github.io/books-futures-explained/4_pin.html
// I cannot figure self-referntial structs in a nice way
#[derive(Debug)]
pub struct Post<'a> {
    original: &'a str,
    pub views: Vec<PostView<'a>>,
    pub lang_list: Vec<&'a str>,
}

#[derive(Debug)]
pub struct PostView<'a> {
    pub lang: Option<&'a str>,
    pub body: Vec<&'a str>,
}

impl<'a> Post<'a> {
    pub fn new_multi_lang(original: &'a str) -> Self {
        let original_len = original.len();
        let split = (0..original_len).split(original);
        // TODO: capacity for 'unique_langs' and 'sections'?
        let mut unique_langs: Vec<&str> = Vec::new();
        let mut sections: Vec<WipPart> = Vec::new();
        for (_row, (range, _)) in split.enumerate() {
            let line = range.of(original);
            // Three cases:
            // 1. Line we are processing is a compile flag
            if let Some(language_str) = find_config_key_end(line) {
                // Extracts post-colon 'api_set_lang: en all jp'
                // Lines may belong to many languages
                let (has_all_tag, langs_given) = analyse_langs(language_str);
                unique_append(&mut unique_langs, &langs_given);

                // Languages may or may not be specified
                let languages = if has_all_tag || langs_given.is_empty() {
                    None
                } else {
                    Some(langs_given)
                };
                let empty_range_after_newline = range.end ..range.end;
                sections.push((languages, empty_range_after_newline));

            // 2. No compile flag as is first line
            } else if sections.is_empty() {
                sections.push((None, range));

            // 3. Continuing an existing section
            } else {
                // Case 2 ensures ths is never negative
                let last = sections.len() - 1;
                sections[last].1.expand(range.end);
            }
        }

        let lang_count = unique_langs.len();
        let p = sections;
        let mut data = Vec::with_capacity(lang_count);

        if lang_count <= 1 {
            let size = p.iter().filter_map(|x| pick_lang(x, "")).count();
            let mut view = Vec::with_capacity(size);
            view.extend(p.iter()
                .filter_map(|x| pick_lang(x, ""))
                .map(|r| r.of(original)));
            data.push(PostView {
                lang: ALL_LANG,
                body: view,
            });
        } else {
            for lang in &unique_langs {
                let size = p.iter().filter_map(|x| pick_lang(x, lang)).count();
                let mut view = Vec::with_capacity(size);
                view.extend(p.iter()
                    .filter_map(|x| pick_lang(x, lang))
                    .map(|r| r.of(original)));
                data.push(PostView {
                    lang: Some(lang),
                    body: view,
                });
            }
        }
        //data.iter().for_each(|a| a.body.iter().for_each(|b| println!("{}", b.of(original_str))));

        Self {
            original,
            views: data,
            lang_list: unique_langs,
        }
    }

    pub fn new_single_lang(text: &'a str) -> Self {
        Self {
            original: text,
            views: vec![PostView {
                lang: ALL_LANG,
                body: vec![text],
            }],
            lang_list: vec![],
        }
    }
}



/******************************************************************************
 * Post helper functions
 ******************************************************************************/
fn pick_lang(entry: &WipPart, pick: &str) -> Option<Range<usize>> {
    let (maybe_all_langs, range) = entry;
    if let Some(lang_list) = maybe_all_langs {
        if lang_list.iter().all(|lang| *lang != pick) {
            return None;
        }
    }
    Some(range.start .. range.end) // duplicate because access by reference
}

fn is_all_lang(tag: &str) -> bool {
    tag == "*" || tag == "ALL"
}

#[inline]
fn analyse_langs(config_str: &str) -> (bool, Vec<&str>) {
    let has_all = config_str.split_whitespace().any(is_all_lang);
    // TODO: add check for valid lang_tag
    let langs = config_str
        .split_whitespace()
        .filter(|lang| !is_all_lang(lang))
        .collect::<Vec<_>>();
    //for lang in &langs {

    //}

    (has_all, langs)
}

#[inline]
fn unique_append<'a>(unique: &mut Vec<&'a str>, to_add: &[&'a str]) {
    let always_add = unique.is_empty();
    for lang in to_add {
        if always_add || unique.iter().all(|l| l != lang) {
            unique.push(lang);
        }
    }
}


fn get_comment_body(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        Some(&trimmed["//".len()..])
    } else {
        None
    }
}


fn find_config_key_end(line: &str) -> Option<&str> {
    get_comment_body(line).and_then(|comment| {
        let body = comment.trim();
        if body.starts_with(API_SET_LANGUAGE) {
            Some(&body[API_SET_LANGUAGE.len()..])
        } else {
            None
        }
    })
}

trait SubstrToRange {
    fn range_within(&self, container: &str) -> Range<usize>;
}

impl SubstrToRange for str {
    fn range_within(&self, container: &str) -> Range<usize> {
        let start = self.as_ptr() as usize - container.as_ptr() as usize;
        start .. start + self.len()
    }
}


#[test]
fn range_within() {
    let a = "     asdf\nsheep ";
    let r = a.trim().range_within(a);
    assert_eq!(&a[r], "asdf\nsheep");
}
use std::ops::Range;

//trait RangeExt: std::slice::SliceIndex<str> {
trait RangeExt {
    fn of<'a>(&self, original: &'a str) -> &'a str;
    fn expand(&mut self, till: usize) -> &mut Self;
    fn split<'a>(&self, original: &'a str) -> RangeSplitInclusive<'a>;
    fn split_over<'a>(&self,
        original: &'a str,
        delimiter: fn(&str) -> RStr,
    ) -> RangeSplitInclusive<'a>;
}


impl RangeExt for RStr {
    fn of<'a>(&self, original: &'a str) -> &'a str  {
        &original[self.start .. self.end]
    }
    fn expand(&mut self, till: usize) -> &mut Self {
        self.end = till;
        self
    }
    fn split<'a>(&self, original: &'a str) -> RangeSplitInclusive<'a> {
        RangeSplitInclusive {
            buffer: &original[self.start .. self.end],
            delimit_by: |substr| {
                let len = substr.len();
                substr.find("\n").map(|i| i+1..i+1).unwrap_or(len..len)
            },
            index: self.start,
        }
    }
    fn split_over<'a>(&self,
        original: &'a str,
        delimit_by: fn(&str) -> RStr,
    ) -> RangeSplitInclusive<'a> {
        RangeSplitInclusive {
            buffer: &original[self.start .. self.end],
            delimit_by,
            index: self.start,
        }
    }
}

struct RangeSplitInclusive<'a> {
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
            let delimiter = rel_delim.start + start..rel_delim.end+start;
            Some((start..start+rel_delim.start, delimiter))
        } else {
            None

        }
    }
}




#[test]
fn split_test() {
    let body = "hello你\n你how\n are you tody 你好嗎" ;
    (2..body.len()-6).split(body).for_each(|a| {
        assert!(a.1.of(body).is_empty());
        println!("{:?} {:?}", a, a.0.of(body));
    });
    println!("===");

    (1..body.len()-6).split_over(body, |substr|
        substr.find("\n").map(|i| i..i+1).unwrap_or(substr.len()..substr.len())
    ).for_each(|a| {
        println!("{:?} {:?} {:?}", a, a.0.of(body), a.1.of(body));
    })
        ;
}
