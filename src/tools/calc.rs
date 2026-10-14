const MAX_INPUT: usize = 200;
const MAX_DEPTH: u32 = 48;

struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: u32,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') {
                v *= self.unary()?;
            } else if self.eat('/') {
                v /= self.unary()?;
            } else if self.eat('%') {
                v %= self.unary()?;
            } else {
                return Some(v);
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat('-') {
            return Some(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.atom()?;
        if self.eat('^') {
            let exp = self.unary()?;
            return Some(base.powf(exp));
        }
        Some(base)
    }

    fn atom(&mut self) -> Option<f64> {
        self.skip_ws();
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let result = self.atom_inner();
        self.depth -= 1;
        result
    }

    fn atom_inner(&mut self) -> Option<f64> {
        let c = self.peek()?;
        if c == '(' {
            self.pos += 1;
            let v = self.expr()?;
            return self.eat(')').then_some(v);
        }
        if c.is_ascii_digit() || c == '.' {
            let start = self.pos;
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                self.pos += 1;
            }
            return self.chars[start..self.pos].iter().collect::<String>().parse().ok();
        }
        if c.is_alphabetic() {
            let start = self.pos;
            while self.peek().is_some_and(char::is_alphanumeric) {
                self.pos += 1;
            }
            let name: String = self.chars[start..self.pos].iter().collect::<String>().to_lowercase();
            return match name.as_str() {
                "pi" | "π" => Some(std::f64::consts::PI),
                "e" => Some(std::f64::consts::E),
                _ => {
                    if !self.eat('(') {
                        return None;
                    }
                    let arg = self.expr()?;
                    if !self.eat(')') {
                        return None;
                    }
                    function(&name, arg)
                }
            };
        }
        None
    }
}

fn function(name: &str, x: f64) -> Option<f64> {
    Some(match name {
        "sqrt" => x.sqrt(),
        "abs" => x.abs(),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "ln" => x.ln(),
        "log" | "lg" => x.log10(),
        "floor" => x.floor(),
        "ceil" => x.ceil(),
        "round" => x.round(),
        _ => return None,
    })
}

fn format(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if v.abs() >= 1e15 || v.abs() < 1e-9 {
        return format!("{v:.8e}");
    }
    let s = format!("{v:.10}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

pub fn evaluate(input: &str) -> Option<String> {
    if input.chars().count() > MAX_INPUT {
        return None;
    }
    let normalized: String = input
        .chars()
        .map(|c| match c {
            '×' | '·' => '*',
            '÷' => '/',
            '−' => '-',
            ',' => '.',
            c => c,
        })
        .collect();
    let has_digit = normalized.chars().any(|c| c.is_ascii_digit());
    let has_op = normalized.chars().skip(1).any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')'))
        || normalized.chars().next().is_some_and(|c| c.is_alphabetic());
    if !has_digit || !has_op {
        return None;
    }
    let mut p = Parser { chars: normalized.chars().collect(), pos: 0, depth: 0 };
    let value = p.expr()?;
    p.skip_ws();
    (p.pos == p.chars.len() && value.is_finite()).then(|| format(value))
}

#[derive(Clone, Copy, PartialEq)]
enum Dim {
    Length,
    Mass,
    Volume,
    Temp,
}

fn unit(name: &str) -> Option<(Dim, f64, &'static str)> {
    Some(match name {
        "mm" => (Dim::Length, 0.001, "mm"),
        "cm" => (Dim::Length, 0.01, "cm"),
        "m" => (Dim::Length, 1.0, "m"),
        "km" => (Dim::Length, 1000.0, "km"),
        "in" | "inch" | "inches" => (Dim::Length, 0.0254, "in"),
        "ft" | "feet" | "foot" => (Dim::Length, 0.3048, "ft"),
        "yd" => (Dim::Length, 0.9144, "yd"),
        "mi" | "mile" | "miles" => (Dim::Length, 1609.344, "mi"),
        "g" => (Dim::Mass, 0.001, "g"),
        "kg" => (Dim::Mass, 1.0, "kg"),
        "t" => (Dim::Mass, 1000.0, "t"),
        "lb" | "lbs" => (Dim::Mass, 0.45359237, "lb"),
        "oz" => (Dim::Mass, 0.028349523125, "oz"),
        "ml" => (Dim::Volume, 0.001, "ml"),
        "l" => (Dim::Volume, 1.0, "l"),
        "gal" => (Dim::Volume, 3.785411784, "gal"),
        "c" | "°c" => (Dim::Temp, 1.0, "°C"),
        "f" | "°f" => (Dim::Temp, 1.0, "°F"),
        "k" => (Dim::Temp, 1.0, "K"),
        _ => return None,
    })
}

fn to_celsius(v: f64, label: &str) -> f64 {
    match label {
        "°F" => (v - 32.0) * 5.0 / 9.0,
        "K" => v - 273.15,
        _ => v,
    }
}

fn from_celsius(v: f64, label: &str) -> f64 {
    match label {
        "°F" => v * 9.0 / 5.0 + 32.0,
        "K" => v + 273.15,
        _ => v,
    }
}

pub fn convert(input: &str) -> Option<String> {
    if input.chars().count() > MAX_INPUT {
        return None;
    }
    let lower = input.trim().to_lowercase().replace("->", " to ").replace(" in ", " to ");
    let (left, to) = lower.split_once(" to ")?;
    let to = to.trim();
    let left = left.trim();
    let split = left.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',' || c == '-'))?;
    let value: f64 = left[..split].replace(',', ".").parse().ok()?;
    let (dim_from, f_from, label_from) = unit(left[split..].trim())?;
    let (dim_to, f_to, label_to) = unit(to)?;
    if dim_from != dim_to {
        return None;
    }
    let result = if dim_from == Dim::Temp {
        from_celsius(to_celsius(value, label_from), label_to)
    } else {
        value * f_from / f_to
    };
    result.is_finite().then(|| format!("{} {} = {} {}", format(value), label_from, format(result), label_to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        assert_eq!(evaluate("2+2").as_deref(), Some("4"));
        assert_eq!(evaluate("2 + 3 * 4").as_deref(), Some("14"));
        assert_eq!(evaluate("(2+3)*4").as_deref(), Some("20"));
        assert_eq!(evaluate("2^3^2").as_deref(), Some("512"));
        assert_eq!(evaluate("-2^2").as_deref(), Some("-4"));
        assert_eq!(evaluate("10/4").as_deref(), Some("2.5"));
        assert_eq!(evaluate("0,1+0,2").as_deref(), Some("0.3"));
        assert_eq!(evaluate("sqrt(16)+abs(-3)").as_deref(), Some("7"));
        assert_eq!(evaluate("7 % 4").as_deref(), Some("3"));
    }

    #[test]
    fn rejects_non_expressions() {
        assert_eq!(evaluate("42"), None);
        assert_eq!(evaluate("hello"), None);
        assert_eq!(evaluate("2+"), None);
        assert_eq!(evaluate("(1+2"), None);
        assert_eq!(evaluate("1/0"), None);
        assert_eq!(evaluate("foo(3)"), None);
    }

    #[test]
    fn hostile_input_is_bounded() {
        let deep = "(".repeat(5000) + "1" + &")".repeat(5000);
        assert_eq!(evaluate(&deep), None);
        let long = "1+".repeat(500) + "1";
        assert_eq!(evaluate(&long), None);
    }

    #[test]
    fn units() {
        assert_eq!(convert("1 km to m").as_deref(), Some("1 km = 1000 m"));
        assert_eq!(convert("100 c to f").as_deref(), Some("100 °C = 212 °F"));
        assert_eq!(convert("1 kg to m"), None);
        assert_eq!(convert("nonsense"), None);
    }
}
