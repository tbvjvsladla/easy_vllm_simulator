//! `json.loads(text, object_pairs_hook=..., parse_int=..., parse_float=...)` as tripwire ② uses it: a value tree
//! (objects keep first-key position and last value, like `dict(pairs)`), a duplicate-key flag and every numeric
//! token's spelling in parse order. Any decode error is `Err` (the original's `ValueError`).
#[derive(Clone, Debug)]
pub enum JV {
    Null,
    Bool(bool),
    Num(String),
    Const(String),
    Str(String),
    Arr(Vec<JV>),
    Obj(Vec<(String, JV)>),
}

impl JV {
    pub fn get(&self, k: &str) -> Option<&JV> {
        if let JV::Obj(v) = self {
            v.iter().find(|(a, _)| a == k).map(|(_, b)| b)
        } else {
            None
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let JV::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

/// Python `==` on decoded JSON values (dicts order-insensitive, int/float by value).
pub fn py_eq(a: &JV, b: &JV) -> bool {
    match (a, b) {
        (JV::Null, JV::Null) => true,
        (JV::Bool(x), JV::Bool(y)) => x == y,
        (JV::Num(x), JV::Num(y)) => x == y || x.parse::<f64>().ok() == y.parse::<f64>().ok(),
        (JV::Bool(x), JV::Num(y)) | (JV::Num(y), JV::Bool(x)) => y.parse::<f64>().ok() == Some(if *x { 1.0 } else { 0.0 }),
        (JV::Const(x), JV::Const(y)) => x == y && x != "NaN",
        (JV::Str(x), JV::Str(y)) => x == y,
        (JV::Arr(x), JV::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
        (JV::Obj(x), JV::Obj(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && py_eq(v, v2)))
        }
        _ => false,
    }
}

pub struct Parsed {
    pub value: JV,
    pub duplicate_keys: bool,
    pub numeric_tokens: Vec<String>,
}

struct P<'a> {
    s: &'a [char],
    i: usize,
    dup: bool,
    nums: Vec<String>,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], ' ' | '\t' | '\n' | '\r') {
            self.i += 1;
        }
    }
    fn lit(&mut self, w: &str) -> bool {
        let n = w.chars().count();
        if self.i + n <= self.s.len() && self.s[self.i..self.i + n].iter().copied().eq(w.chars()) {
            self.i += n;
            true
        } else {
            false
        }
    }
    fn val(&mut self, depth: usize) -> Result<JV, ()> {
        if depth > 900 {
            return Err(());
        }
        self.ws();
        let c = *self.s.get(self.i).ok_or(())?;
        match c {
            '{' => {
                self.i += 1;
                let mut v: Vec<(String, JV)> = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&'}') {
                    self.i += 1;
                    return Ok(JV::Obj(v));
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&'"') {
                        return Err(());
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.s.get(self.i) != Some(&':') {
                        return Err(());
                    }
                    self.i += 1;
                    let x = self.val(depth + 1)?;
                    if let Some(e) = v.iter_mut().find(|(a, _)| *a == k) {
                        self.dup = true;
                        e.1 = x;
                    } else {
                        v.push((k, x));
                    }
                    self.ws();
                    match self.s.get(self.i) {
                        Some(',') => self.i += 1,
                        Some('}') => {
                            self.i += 1;
                            return Ok(JV::Obj(v));
                        }
                        _ => return Err(()),
                    }
                }
            }
            '[' => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&']') {
                    self.i += 1;
                    return Ok(JV::Arr(v));
                }
                loop {
                    v.push(self.val(depth + 1)?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(',') => self.i += 1,
                        Some(']') => {
                            self.i += 1;
                            return Ok(JV::Arr(v));
                        }
                        _ => return Err(()),
                    }
                }
            }
            '"' => Ok(JV::Str(self.string()?)),
            'n' if self.lit("null") => Ok(JV::Null),
            't' if self.lit("true") => Ok(JV::Bool(true)),
            'f' if self.lit("false") => Ok(JV::Bool(false)),
            'N' if self.lit("NaN") => Ok(JV::Const("NaN".into())),
            'I' if self.lit("Infinity") => Ok(JV::Const("Infinity".into())),
            '-' if self.s.get(self.i + 1) == Some(&'I') && self.lit("-Infinity") => Ok(JV::Const("-Infinity".into())),
            _ => self.number(),
        }
    }
    fn number(&mut self) -> Result<JV, ()> {
        let st = self.i;
        let d = |p: &P, i: usize| p.s.get(i).map_or(false, |c| c.is_ascii_digit());
        if self.s.get(self.i) == Some(&'-') {
            self.i += 1;
        }
        if self.s.get(self.i) == Some(&'0') {
            self.i += 1;
        } else if self.s.get(self.i).map_or(false, |c| ('1'..='9').contains(c)) {
            while d(self, self.i) {
                self.i += 1;
            }
        } else {
            return Err(());
        }
        if self.s.get(self.i) == Some(&'.') && d(self, self.i + 1) {
            self.i += 1;
            while d(self, self.i) {
                self.i += 1;
            }
        }
        if matches!(self.s.get(self.i), Some('e' | 'E')) {
            let mut j = self.i + 1;
            if matches!(self.s.get(j), Some('+' | '-')) {
                j += 1;
            }
            if d(self, j) {
                self.i = j;
                while d(self, self.i) {
                    self.i += 1;
                }
            }
        }
        let t: String = self.s[st..self.i].iter().collect();
        self.nums.push(t.clone());
        Ok(JV::Num(t))
    }
    fn string(&mut self) -> Result<String, ()> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = *self.s.get(self.i).ok_or(())?;
            self.i += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = *self.s.get(self.i).ok_or(())?;
                    self.i += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let h: String = self.s.get(self.i..self.i + 4).ok_or(())?.iter().collect();
                            let mut v = u32::from_str_radix(&h, 16).map_err(|_| ())?;
                            self.i += 4;
                            if (0xd800..0xdc00).contains(&v) && self.s.get(self.i) == Some(&'\\') && self.s.get(self.i + 1) == Some(&'u') {
                                let h2: String = self.s.get(self.i + 2..self.i + 6).ok_or(())?.iter().collect();
                                if let Ok(lo) = u32::from_str_radix(&h2, 16) {
                                    if (0xdc00..0xe000).contains(&lo) {
                                        v = 0x10000 + ((v - 0xd800) << 10) + (lo - 0xdc00);
                                        self.i += 6;
                                    }
                                }
                            }
                            out.push(char::from_u32(v).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(()),
                    }
                }
                c if (c as u32) < 0x20 => return Err(()),
                c => out.push(c),
            }
        }
    }
}

pub fn loads(text: &str) -> Result<Parsed, ()> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = P { s: &chars, i: 0, dup: false, nums: Vec::new() };
    let v = p.val(0)?;
    p.ws();
    if p.i != chars.len() {
        return Err(());
    }
    Ok(Parsed { value: v, duplicate_keys: p.dup, numeric_tokens: p.nums })
}
