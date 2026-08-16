#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Code(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Link {
        target: String,
        content: Vec<Inline>,
    },
    Image {
        target: String,
        alt: String,
    },
    SoftBreak,
    HardBreak,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    /// `None` for a plain bullet, `Some(checked)` for a task item.
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub align: Vec<Align>,
    pub head: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// `slug` is empty until `outline::build` fills it in (Task 10).
    Heading {
        level: u8,
        content: Vec<Inline>,
        slug: String,
    },
    Paragraph(Vec<Inline>),
    Code {
        lang: Option<String>,
        text: String,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<ListItem>,
    },
    Table(Table),
    Rule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    pub level: u8,
    pub text: String,
    pub slug: String,
    pub block_index: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub outline: Vec<OutlineEntry>,
}
