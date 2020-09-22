use crate::custom_errors::ParseError;
use crate::traits::{BoolExt, RStr, RangeExt, VecExt};
use std::borrow::Cow;
use std::mem::replace;

type WipPart<'a> = (Option<Vec<&'a str>>, RStr);
const API_SET_LANGUAGE: &str = "api_set_lang:";
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
    pub fn new(text: &'a str, comment_marker: &str) -> Result<Self, ParseError<'a>> {
        // 'lang_max_count' will count all duplicates (which is the common case)
        // e.g. api_set_lang: en jp
        //      api_set_lang: ALL
        //      api_set_lang: en
        // counts four
        // It might just be better to let 'unique_langs' auto size
        let (section_count, lang_max_count) = text
            .lines()
            // TODO: rename find_config_key_end
            .filter_map(|line| find_config_key_end(line, comment_marker))
            .map(|body| body.split_whitespace().count())
            .fold((1, 0), |(count, sum), to_add| (count + 1, sum + to_add));

        let mut unique_langs = Vec::with_capacity(lang_max_count);
        let mut parts = Vec::with_capacity(section_count);
        let mut toadd_lang = None;
        let mut toadd_rstr = 0..0;
        for (row, (range, _)) in (0..text.len()).split(text).enumerate() {
            let line = range.of(text);
            if let Some(lang_str) = find_config_key_end(line, comment_marker) {
                // e.g. 'api_set_lang: en ALL jp' -> 'en all jp'
                // May be both all and list languages
                let (has_all_tag, langs_given) =
                    analyse_langs(lang_str).map_err(|err| (row, line, Cow::Owned(err)))?;
                unique_append(&mut unique_langs, &langs_given);

                let languages = (!has_all_tag && !langs_given.is_empty()).to_some(langs_given);
                parts.push_and_check((
                    replace(&mut toadd_lang, languages),
                    replace(&mut toadd_rstr, range.end..range.end),
                ));
            } else {
                toadd_rstr.expand(range.end);
            }
        }
        parts.push_and_check((toadd_lang, toadd_rstr));

        // Transpose 'parts' from by parts-by-langs to langs-by-parts (PostView)
        let lang_count = unique_langs.len();
        let mut view_list = Vec::with_capacity(std::cmp::max(lang_count, 1));
        if unique_langs.is_empty() {
            // For only all lang case, still filter out api_set_lang markup
            // i.e. use 'parts' not the original 'text'
            let mut parts_to_str = Vec::with_capacity(parts.len());
            parts_to_str.extend(parts.iter().map(|(_, range)| range.of(text)));
            view_list.push_and_check(PostView {
                lang: None,
                body: parts_to_str,
            });
        } else {
            for lang in &unique_langs {
                let size = parts.iter().filter_map(|x| pick_lang(x, lang)).count();
                let mut view = Vec::with_capacity(size);
                view.extend(
                    parts
                        .iter()
                        .filter_map(|x| pick_lang(x, lang))
                        .map(|r| r.of(text)),
                );

                view_list.push_and_check(PostView {
                    lang: Some(lang),
                    body: view,
                });
            }
        }

        Ok(Self {
            original: text,
            views: view_list,
            lang_list: unique_langs,
        })
    }
}

/******************************************************************************
 * Post helper functions
 ******************************************************************************/
fn pick_lang(entry: &WipPart, pick: &str) -> Option<RStr> {
    let (maybe_all_langs, range) = entry;
    if let Some(lang_list) = maybe_all_langs {
        if lang_list.iter().all(|lang| *lang != pick) {
            return None;
        }
    }
    Some(range.start..range.end) // duplicate because access by reference
}

fn is_all_lang(tag: &str) -> bool {
    tag == "*" || tag == "ALL"
}

#[inline]
fn analyse_langs(lang_line: &str) -> Result<(bool, Vec<&str>), String> {
    let has_all = lang_line.split_whitespace().any(is_all_lang);
    let mut langs = Vec::with_capacity(lang_line.split_whitespace().count());
    for l in lang_line.split_whitespace().filter(|l| !is_all_lang(l)) {
        langs.push_and_check(l);
        if l.contains(&['/', '\\'][..]) {
            return Err(format!("{:?} is an invalid tag.", l));
        }
    }

    Ok((has_all, langs))
}

#[inline]
fn unique_append<'a>(unique: &mut Vec<&'a str>, to_add: &[&'a str]) {
    let always_add = unique.is_empty();
    for lang in to_add {
        if always_add || unique.iter().all(|l| l != lang) {
            unique.push_and_check(lang);
        }
    }
}

#[inline]
fn find_config_key_end<'a>(line: &'a str, comment_marker: &str) -> Option<&'a str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with(comment_marker) {
        let comment_body = trimmed[comment_marker.len()..].trim();
        if comment_body.starts_with(API_SET_LANGUAGE) {
            return Some(&comment_body[API_SET_LANGUAGE.len()..]);
        }
    }
    None
}

trait SubstrToRange {
    fn range_within(&self, container: &str) -> RStr;
}

impl SubstrToRange for str {
    fn range_within(&self, container: &str) -> RStr {
        let start = self.as_ptr() as usize - container.as_ptr() as usize;
        start..start + self.len()
    }
}

#[test]
fn range_within() {
    let a = "     asdf\nsheep ";
    let r = a.trim().range_within(a);
    assert_eq!(&a[r], "asdf\nsheep");
}
