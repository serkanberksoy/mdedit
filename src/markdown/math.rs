//! Math (X-01, X-02): LaTeX as Unicode, as near as text gets. `$e =
//! mc^2$` reads `e = mc²`, `\sum_{i=1}^{n}` reads `∑ᵢ₌₁ⁿ`. Greek letters,
//! operators, relations and arrows become their symbols; `^` / `_` become
//! superscripts / subscripts where Unicode has them (else `^(…)` /
//! `_(…)`); `\frac{a}{b}` is `a/b`, `\sqrt{x}` is `√x`; `\mathbb{R}` is
//! `ℝ`; `\text{…}` is its text. A command it doesn't know stays as written.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// How math shows.
pub const STYLE: Style = Style::new()
    .fg(Color::LightMagenta)
    .add_modifier(Modifier::ITALIC);

/// A line of a `$$` block (X-02): its `$$` dimmed, the math as Unicode (a
/// one-line `$$ … $$` too).
pub fn render_line(line: &str) -> Line<'static> {
    let trimmed = line.trim();
    if trimmed == "$$" {
        return Line::from(Span::styled(
            line.to_string(),
            Style::new().fg(Color::DarkGray),
        ));
    }
    let latex = trimmed.strip_prefix("$$").unwrap_or(trimmed);
    let latex = latex.strip_suffix("$$").unwrap_or(latex);
    Line::from(Span::styled(to_unicode(latex.trim()), STYLE))
}

/// `latex` as Unicode.
pub fn to_unicode(latex: &str) -> String {
    let chars: Vec<char> = latex.chars().collect();
    let mut at = 0;
    let out = convert(&chars, &mut at, false);
    // Spaces as LaTeX sees them: one is enough.
    let mut tidy = String::new();
    for c in out.chars() {
        if !(c == ' ' && tidy.ends_with(' ')) {
            tidy.push(c);
        }
    }
    tidy
}

/// Converts from `at` to the end, or (`group`) to the `}` closing it.
fn convert(chars: &[char], at: &mut usize, group: bool) -> String {
    let mut out = String::new();
    while *at < chars.len() {
        let c = chars[*at];
        *at += 1;
        match c {
            '}' if group => return out,
            '{' => out.push_str(&convert(chars, at, true)),
            '^' | '_' => {
                let arg = argument(chars, at);
                out.push_str(&script(&arg, c == '^'));
            }
            '\\' => out.push_str(&command(chars, at)),
            c => out.push(c),
        }
    }
    out
}

/// The argument at `at`, converted: a `{group}`, a command, or one char.
fn argument(chars: &[char], at: &mut usize) -> String {
    while chars.get(*at) == Some(&' ') {
        *at += 1;
    }
    match chars.get(*at) {
        Some('{') => {
            *at += 1;
            convert(chars, at, true)
        }
        Some('\\') => {
            *at += 1;
            command(chars, at)
        }
        Some(&c) => {
            *at += 1;
            c.to_string()
        }
        None => String::new(),
    }
}

/// A `{group}` at `at` as written (for a command kept as it is).
fn raw_group(chars: &[char], at: &mut usize) -> String {
    if chars.get(*at) != Some(&'{') {
        return String::new();
    }
    let start = *at;
    let mut depth = 0;
    while *at < chars.len() {
        match chars[*at] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    *at += 1;
                    break;
                }
            }
            _ => {}
        }
        *at += 1;
    }
    chars[start..*at].iter().collect()
}

/// `text` as a superscript (`up`) or subscript: each char's own where
/// Unicode has them all, else `^(text)` / `_(text)`.
fn script(text: &str, up: bool) -> String {
    let mapped: Option<String> = text
        .chars()
        .filter(|c| *c != ' ')
        .map(|c| if up { superscript(c) } else { subscript(c) })
        .collect();
    match mapped {
        Some(m) if !m.is_empty() => m,
        _ => {
            let mark = if up { '^' } else { '_' };
            if text.chars().count() == 1 {
                format!("{mark}{text}")
            } else {
                format!("{mark}({text})")
            }
        }
    }
}

fn superscript(c: char) -> Option<char> {
    const FROM: &str = "0123456789+-=()abcdefghijklmnoprstuvwxyzABDEGHIJKLMNOPRTUVW";
    const TO: &str = "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖʳˢᵗᵘᵛʷˣʸᶻᴬᴮᴰᴱᴳᴴᴵᴶᴷᴸᴹᴺᴼᴾᴿᵀᵁⱽᵂ";
    FROM.chars()
        .position(|f| f == c)
        .and_then(|i| TO.chars().nth(i))
}

fn subscript(c: char) -> Option<char> {
    const FROM: &str = "0123456789+-=()aehijklmnoprstuvx";
    const TO: &str = "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ";
    FROM.chars()
        .position(|f| f == c)
        .and_then(|i| TO.chars().nth(i))
}

/// `text` in parentheses unless it's one simple term (`2`, `x`, `ab`).
fn term(text: &str) -> String {
    let text = text.trim();
    if text.chars().all(|c| c.is_alphanumeric() || c == '.') || text.chars().count() == 1 {
        text.to_string()
    } else {
        format!("({text})")
    }
}

/// The command after a `\` at `at`, converted.
fn command(chars: &[char], at: &mut usize) -> String {
    let start = *at;
    while chars.get(*at).is_some_and(|c| c.is_ascii_alphabetic()) {
        *at += 1;
    }
    if *at == start {
        // One char: an escaped symbol, or a space.
        let Some(&c) = chars.get(*at) else {
            return "\\".into();
        };
        *at += 1;
        return match c {
            ',' | ';' | ':' | ' ' | '\\' => " ".into(),
            '!' => String::new(),
            c => c.to_string(),
        };
    }
    let name: String = chars[start..*at].iter().collect();
    match name.as_str() {
        "frac" | "dfrac" | "tfrac" => {
            let top = argument(chars, at);
            let bottom = argument(chars, at);
            format!("{}/{}", term(&top), term(&bottom))
        }
        "sqrt" => {
            let mut root = '√';
            if chars.get(*at) == Some(&'[') {
                let end = chars[*at..].iter().position(|&c| c == ']').map(|e| *at + e);
                if let Some(end) = end {
                    let n: String = chars[*at + 1..end].iter().collect();
                    root = match n.trim() {
                        "3" => '∛',
                        "4" => '∜',
                        _ => '√',
                    };
                    *at = end + 1;
                }
            }
            format!("{root}{}", term(&argument(chars, at)))
        }
        "text" | "textrm" | "textit" | "textbf" | "mbox" => {
            let raw = raw_group(chars, at);
            raw.trim_start_matches('{')
                .trim_end_matches('}')
                .to_string()
        }
        "mathrm" | "mathit" | "mathbf" | "mathsf" | "mathcal" | "operatorname" | "boldsymbol" => {
            argument(chars, at)
        }
        "mathbb" => argument(chars, at)
            .chars()
            .map(|c| match c {
                'R' => 'ℝ',
                'N' => 'ℕ',
                'Z' => 'ℤ',
                'Q' => 'ℚ',
                'C' => 'ℂ',
                'P' => 'ℙ',
                'H' => 'ℍ',
                c => c,
            })
            .collect(),
        "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" => {
            // The delimiter stays; `\left.` is none.
            if chars.get(*at) == Some(&'.') {
                *at += 1;
            }
            String::new()
        }
        "quad" | "qquad" => " ".into(),
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "log" | "ln" | "exp" | "lim" | "max"
        | "min" | "sup" | "inf" | "det" | "gcd" | "arg" | "deg" | "dim" | "ker" | "mod"
        | "arcsin" | "arccos" | "arctan" | "sinh" | "cosh" | "tanh" => name,
        _ => match symbol(&name) {
            Some(s) => s.to_string(),
            // Unknown: as written, with its argument.
            None => format!("\\{name}{}", raw_group(chars, at)),
        },
    }
}

/// A command that is one symbol.
fn symbol(name: &str) -> Option<&'static str> {
    const SYMBOLS: &[(&str, &str)] = &[
        ("alpha", "α"),
        ("beta", "β"),
        ("gamma", "γ"),
        ("delta", "δ"),
        ("epsilon", "ϵ"),
        ("varepsilon", "ε"),
        ("zeta", "ζ"),
        ("eta", "η"),
        ("theta", "θ"),
        ("vartheta", "ϑ"),
        ("iota", "ι"),
        ("kappa", "κ"),
        ("lambda", "λ"),
        ("mu", "μ"),
        ("nu", "ν"),
        ("xi", "ξ"),
        ("pi", "π"),
        ("varpi", "ϖ"),
        ("rho", "ρ"),
        ("varrho", "ϱ"),
        ("sigma", "σ"),
        ("varsigma", "ς"),
        ("tau", "τ"),
        ("upsilon", "υ"),
        ("phi", "ϕ"),
        ("varphi", "φ"),
        ("chi", "χ"),
        ("psi", "ψ"),
        ("omega", "ω"),
        ("Gamma", "Γ"),
        ("Delta", "Δ"),
        ("Theta", "Θ"),
        ("Lambda", "Λ"),
        ("Xi", "Ξ"),
        ("Pi", "Π"),
        ("Sigma", "Σ"),
        ("Upsilon", "Υ"),
        ("Phi", "Φ"),
        ("Psi", "Ψ"),
        ("Omega", "Ω"),
        ("sum", "∑"),
        ("prod", "∏"),
        ("coprod", "∐"),
        ("int", "∫"),
        ("iint", "∬"),
        ("iiint", "∭"),
        ("oint", "∮"),
        ("infty", "∞"),
        ("partial", "∂"),
        ("nabla", "∇"),
        ("pm", "±"),
        ("mp", "∓"),
        ("times", "×"),
        ("div", "÷"),
        ("cdot", "·"),
        ("ast", "∗"),
        ("star", "⋆"),
        ("circ", "∘"),
        ("bullet", "•"),
        ("oplus", "⊕"),
        ("otimes", "⊗"),
        ("leq", "≤"),
        ("le", "≤"),
        ("geq", "≥"),
        ("ge", "≥"),
        ("neq", "≠"),
        ("ne", "≠"),
        ("approx", "≈"),
        ("equiv", "≡"),
        ("sim", "∼"),
        ("simeq", "≃"),
        ("cong", "≅"),
        ("propto", "∝"),
        ("ll", "≪"),
        ("gg", "≫"),
        ("in", "∈"),
        ("notin", "∉"),
        ("ni", "∋"),
        ("subset", "⊂"),
        ("supset", "⊃"),
        ("subseteq", "⊆"),
        ("supseteq", "⊇"),
        ("cup", "∪"),
        ("cap", "∩"),
        ("setminus", "∖"),
        ("emptyset", "∅"),
        ("varnothing", "∅"),
        ("forall", "∀"),
        ("exists", "∃"),
        ("nexists", "∄"),
        ("neg", "¬"),
        ("lnot", "¬"),
        ("land", "∧"),
        ("wedge", "∧"),
        ("lor", "∨"),
        ("vee", "∨"),
        ("to", "→"),
        ("rightarrow", "→"),
        ("leftarrow", "←"),
        ("gets", "←"),
        ("leftrightarrow", "↔"),
        ("Rightarrow", "⇒"),
        ("Leftarrow", "⇐"),
        ("Leftrightarrow", "⇔"),
        ("iff", "⇔"),
        ("implies", "⟹"),
        ("mapsto", "↦"),
        ("uparrow", "↑"),
        ("downarrow", "↓"),
        ("ldots", "…"),
        ("dots", "…"),
        ("cdots", "⋯"),
        ("vdots", "⋮"),
        ("ddots", "⋱"),
        ("prime", "′"),
        ("degree", "°"),
        ("angle", "∠"),
        ("perp", "⊥"),
        ("parallel", "∥"),
        ("mid", "∣"),
        ("langle", "⟨"),
        ("rangle", "⟩"),
        ("lfloor", "⌊"),
        ("rfloor", "⌋"),
        ("lceil", "⌈"),
        ("rceil", "⌉"),
        ("hbar", "ℏ"),
        ("ell", "ℓ"),
        ("Re", "ℜ"),
        ("Im", "ℑ"),
        ("aleph", "ℵ"),
        ("top", "⊤"),
        ("bot", "⊥"),
        ("vert", "|"),
        ("Vert", "‖"),
        ("backslash", "\\"),
        ("triangle", "△"),
        ("square", "□"),
        ("checkmark", "✓"),
        ("dagger", "†"),
        ("therefore", "∴"),
        ("because", "∵"),
        ("cdotp", "·"),
        ("colon", ":"),
        ("lbrace", "{"),
        ("rbrace", "}"),
    ];
    SYMBOLS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Where inline math (`$…$`) starts in `rest` (which starts with `$`):
/// its length with both `$`, if it is math. As Obsidian reads it: the
/// opening `$` isn't followed by a space, the closing one isn't after a
/// space nor before a digit (`$5 and $10` isn't math), and `$$` isn't
/// inline math.
pub fn inline_len(rest: &str) -> Option<usize> {
    let inner = rest.strip_prefix('$')?;
    let first = inner.chars().next()?;
    if first == '$' || first.is_whitespace() {
        return None;
    }
    let mut prev = first;
    for (i, c) in inner.char_indices().skip(1) {
        if c == '$' && prev != '\\' {
            let after = inner[i + 1..].chars().next();
            if !prev.is_whitespace() && !after.is_some_and(|a| a.is_ascii_digit()) {
                return Some(1 + i + 1);
            }
        }
        prev = c;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_scripts_and_commands() {
        assert_eq!(to_unicode("e = mc^2"), "e = mc²");
        assert_eq!(to_unicode(r"\alpha + \beta"), "α + β");
        assert_eq!(
            to_unicode(r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}"),
            "∑ᵢ₌₁ⁿ i = (n(n+1))/2"
        );
        assert_eq!(to_unicode(r"x_1, x_2 \leq \infty"), "x₁, x₂ ≤ ∞");
        assert_eq!(to_unicode(r"\sqrt{2} \approx 1.41"), "√2 ≈ 1.41");
        assert_eq!(to_unicode(r"\sqrt{a+b}"), "√(a+b)");
        assert_eq!(to_unicode(r"\frac{1}{2}"), "1/2");
        assert_eq!(to_unicode(r"\forall x \in \mathbb{R}"), "∀ x ∈ ℝ");
        assert_eq!(
            to_unicode(r"f(x) \to 0 \text{ as } x \to \infty"),
            "f(x) → 0 as x → ∞"
        );
        assert_eq!(to_unicode(r"\left( a \cdot b \right)"), "( a · b )");
        assert_eq!(
            to_unicode(r"e^{i\pi} + 1 = 0"),
            "e^(iπ) + 1 = 0",
            "no superscript π"
        );
        assert_eq!(
            to_unicode(r"a \pmod{b}"),
            r"a \pmod{b}",
            "unknown: as written"
        );
        assert_eq!(to_unicode(r"\{1, 2\}"), "{1, 2}");
        assert_eq!(to_unicode(r"A \times B \neq \emptyset"), "A × B ≠ ∅");
    }

    #[test]
    fn inline_math_as_obsidian_reads_it() {
        assert_eq!(inline_len("$e = mc^2$ and"), Some(10));
        assert_eq!(inline_len("$5 and $10 are"), None, "a price");
        assert_eq!(inline_len("$ x$"), None, "a space after the opening");
        assert_eq!(inline_len("$x $"), None, "a space before the closing");
        assert_eq!(inline_len("$$x$$"), None, "block math");
        assert_eq!(inline_len("$x$1"), None, "a digit after the closing");
        assert_eq!(inline_len("$a\\$b$"), Some(6), "an escaped $ inside");
    }
}
