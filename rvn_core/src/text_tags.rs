use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct RichTextSegment {
    pub text: String,
    pub color: Option<String>,
    pub speed: Option<f32>,
    pub shake: bool,
    pub pause_after: Option<f32>,
}

impl RichTextSegment {
    fn new(text: String, style: &TextStyleState) -> Self {
        Self {
            text,
            color: style.color.clone(),
            speed: style.speed,
            shake: style.shake,
            pause_after: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichText {
    pub segments: Vec<RichTextSegment>,
}

impl RichText {
    pub fn plain_text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextTagError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for TextTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} à l'offset {}", self.message, self.offset)
    }
}

impl std::error::Error for TextTagError {}

#[derive(Debug, Clone, PartialEq)]
enum OpenTag {
    Color(String),
    Speed(f32),
    Shake,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct TextStyleState {
    color: Option<String>,
    speed: Option<f32>,
    shake: bool,
}

pub fn parse_text_tags(input: &str) -> Result<RichText, TextTagError> {
    let mut segments = Vec::new();
    let mut stack = Vec::new();
    let mut style = TextStyleState::default();
    let mut cursor = 0;

    while let Some(rel_start) = input[cursor..].find('{') {
        let start = cursor + rel_start;
        push_text(&mut segments, &input[cursor..start], &style);

        let Some(rel_end) = input[start..].find('}') else {
            push_text(&mut segments, &input[start..], &style);
            return Ok(RichText { segments });
        };
        let end = start + rel_end;
        let tag = &input[start + 1..end];
        apply_tag(tag, start, &mut stack, &mut style, &mut segments)?;
        cursor = end + 1;
    }

    push_text(&mut segments, &input[cursor..], &style);

    if let Some((tag, offset)) = stack.last() {
        return Err(TextTagError {
            offset: *offset,
            message: format!("Balise {} non refermée", open_tag_display(tag)),
        });
    }

    Ok(RichText { segments })
}

fn push_text(segments: &mut Vec<RichTextSegment>, text: &str, style: &TextStyleState) {
    if text.is_empty() {
        return;
    }
    segments.push(RichTextSegment::new(text.to_string(), style));
}

fn apply_tag(
    tag: &str,
    offset: usize,
    stack: &mut Vec<(OpenTag, usize)>,
    style: &mut TextStyleState,
    segments: &mut Vec<RichTextSegment>,
) -> Result<(), TextTagError> {
    if let Some(value) = tag.strip_prefix("color=") {
        if !is_valid_hex_color(value) {
            return Err(err(offset, format!("Couleur invalide dans {{{tag}}}")));
        }
        stack.push((OpenTag::Color(value.to_string()), offset));
        style.color = Some(value.to_string());
        return Ok(());
    }

    if let Some(value) = tag.strip_prefix("pause=") {
        let pause = parse_positive_float(value)
            .ok_or_else(|| err(offset, format!("Valeur invalide dans {{{tag}}}")))?;
        if let Some(last) = segments.last_mut() {
            last.pause_after = Some(pause);
        } else {
            let mut segment = RichTextSegment::new(String::new(), style);
            segment.pause_after = Some(pause);
            segments.push(segment);
        }
        return Ok(());
    }

    if let Some(value) = tag.strip_prefix("speed=") {
        let speed = parse_positive_float(value)
            .ok_or_else(|| err(offset, format!("Valeur invalide dans {{{tag}}}")))?;
        stack.push((OpenTag::Speed(speed), offset));
        style.speed = Some(speed);
        return Ok(());
    }

    match tag {
        "shake" => {
            stack.push((OpenTag::Shake, offset));
            style.shake = true;
            Ok(())
        }
        "/color" => close_tag(TagKind::Color, offset, stack, style),
        "/speed" => close_tag(TagKind::Speed, offset, stack, style),
        "/shake" => close_tag(TagKind::Shake, offset, stack, style),
        _ if tag.starts_with('/') => Err(err(
            offset,
            format!("Balise {{{tag}}} sans ouverture correspondante"),
        )),
        _ => Err(err(offset, format!("Tag texte inconnu : {{{tag}}}"))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Color,
    Speed,
    Shake,
}

fn close_tag(
    expected: TagKind,
    offset: usize,
    stack: &mut Vec<(OpenTag, usize)>,
    style: &mut TextStyleState,
) -> Result<(), TextTagError> {
    let Some((actual, _)) = stack.pop() else {
        return Err(err(
            offset,
            format!(
                "Balise {{/{}}} sans ouverture correspondante",
                open_tag_name(&expected)
            ),
        ));
    };

    if tag_kind(&actual) != expected {
        return Err(err(
            offset,
            format!("Balise {{/{}}} inattendue", open_tag_name(&expected)),
        ));
    }

    rebuild_style(stack, style);
    Ok(())
}

fn rebuild_style(stack: &[(OpenTag, usize)], style: &mut TextStyleState) {
    *style = TextStyleState::default();
    for (tag, _) in stack {
        match tag {
            OpenTag::Color(color) => style.color = Some(color.clone()),
            OpenTag::Speed(speed) => style.speed = Some(*speed),
            OpenTag::Shake => style.shake = true,
        }
    }
}

fn is_valid_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_positive_float(value: &str) -> Option<f32> {
    let parsed = value.parse::<f32>().ok()?;
    (parsed > 0.0 && parsed.is_finite()).then_some(parsed)
}

fn tag_kind(tag: &OpenTag) -> TagKind {
    match tag {
        OpenTag::Color(_) => TagKind::Color,
        OpenTag::Speed(_) => TagKind::Speed,
        OpenTag::Shake => TagKind::Shake,
    }
}

fn open_tag_name(tag: &TagKind) -> &'static str {
    match tag {
        TagKind::Color => "color",
        TagKind::Speed => "speed",
        TagKind::Shake => "shake",
    }
}

fn open_tag_display(tag: &OpenTag) -> &'static str {
    match tag {
        OpenTag::Color(_) => "{color}",
        OpenTag::Speed(_) => "{speed}",
        OpenTag::Shake => "{shake}",
    }
}

fn err(offset: usize, message: String) -> TextTagError {
    TextTagError { offset, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_text() {
        let rich = parse_text_tags("Bonjour.").unwrap();
        assert_eq!(rich.plain_text(), "Bonjour.");
        assert_eq!(rich.segments.len(), 1);
    }

    #[test]
    fn parses_color() {
        let rich = parse_text_tags("Je suis {color=#ff0000}désolée{/color}.").unwrap();
        assert_eq!(rich.plain_text(), "Je suis désolée.");
        assert_eq!(rich.segments[1].color.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn parses_pause() {
        let rich = parse_text_tags("Attends{pause=0.5}...").unwrap();
        assert_eq!(rich.segments[0].pause_after, Some(0.5));
        assert_eq!(rich.plain_text(), "Attends...");
    }

    #[test]
    fn parses_speed() {
        let rich = parse_text_tags("{speed=0.5}Très lent{/speed} puis normal.").unwrap();
        assert_eq!(rich.segments[0].speed, Some(0.5));
        assert_eq!(rich.segments[1].speed, None);
    }

    #[test]
    fn parses_shake() {
        let rich = parse_text_tags("{shake}Danger{/shake}").unwrap();
        assert!(rich.segments[0].shake);
    }

    #[test]
    fn rejects_unknown_tag() {
        assert!(parse_text_tags("{wave}x{/wave}").is_err());
    }

    #[test]
    fn rejects_invalid_color() {
        assert!(parse_text_tags("{color=red}x{/color}").is_err());
    }

    #[test]
    fn rejects_invalid_pause() {
        assert!(parse_text_tags("{pause=abc}").is_err());
    }

    #[test]
    fn rejects_invalid_speed() {
        assert!(parse_text_tags("{speed=0}x{/speed}").is_err());
    }

    #[test]
    fn rejects_invalid_closing_tag() {
        assert!(parse_text_tags("{/color}").is_err());
    }

    #[test]
    fn rejects_unclosed_tag() {
        assert!(parse_text_tags("{shake}Danger").is_err());
    }
}
