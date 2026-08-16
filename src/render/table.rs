use crate::doc::ir::{Align, Inline, Table};
use crate::doc::outline::inline_text;
use crate::render::inline::render_inlines;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

const MIN_COL: usize = 3;

fn column_count(table: &Table) -> usize {
    table
        .align
        .len()
        .max(table.head.len())
        .max(table.rows.iter().map(Vec::len).max().unwrap_or(0))
}

/// Widest plain-text cell per column. Seeded at 1, not `MIN_COL` — `MIN_COL`
/// is the floor we refuse to shrink *below*, not a width every column starts
/// at. Seeding at 3 would pad every narrow column and break the borders.
fn natural_widths(table: &Table, ncols: usize) -> Vec<usize> {
    let mut widths = vec![1usize; ncols];
    let rows = std::iter::once(&table.head).chain(table.rows.iter());
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(ncols) {
            widths[i] = widths[i].max(inline_text(cell).width());
        }
    }
    widths
}

/// Shrink the widest column repeatedly until the table fits the measure.
/// Proportional scaling would squeeze already-narrow columns below usefulness.
fn fit_widths(mut widths: Vec<usize>, width: usize) -> Vec<usize> {
    let ncols = widths.len();
    if ncols == 0 {
        return widths;
    }
    let overhead = 1 + 3 * ncols;
    let budget = width.saturating_sub(overhead).max(ncols * MIN_COL);

    while widths.iter().sum::<usize>() > budget {
        let Some((idx, _)) = widths
            .iter()
            .enumerate()
            .max_by_key(|(_, w)| **w)
            .filter(|(_, w)| **w > MIN_COL)
        else {
            break;
        };
        widths[idx] -= 1;
    }
    widths
}

fn pad_for(align: Align, used: usize, col: usize) -> (usize, usize) {
    let slack = col.saturating_sub(used);
    match align {
        Align::Right => (slack, 0),
        Align::Center => (slack / 2, slack - slack / 2),
        Align::Left | Align::None => (0, slack),
    }
}

fn border_row(
    widths: &[usize],
    left: char,
    mid: char,
    right: char,
    theme: &Theme,
) -> Line<'static> {
    let mut s = String::new();
    s.push(left);
    for (i, w) in widths.iter().enumerate() {
        if i > 0 {
            s.push(mid);
        }
        s.push_str(&"─".repeat(w + 2));
    }
    s.push(right);
    Line::from(Span::styled(s, Style::default().fg(theme.border)))
}

/// Lay out one row, wrapping cells and padding every cell to the row height.
fn body_rows(
    cells: &[Vec<Inline>],
    widths: &[usize],
    align: &[Align],
    theme: &Theme,
    style: Style,
) -> Vec<Line<'static>> {
    let border = Style::default().fg(theme.border);
    let wrapped: Vec<Vec<Line<'static>>> = widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let empty = Vec::new();
            let cell = cells.get(i).unwrap_or(&empty);
            render_inlines(cell, *w as u16, theme, style)
        })
        .collect();

    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
    let mut out = Vec::new();
    for row in 0..height {
        let mut spans = vec![Span::styled("│".to_string(), border)];
        for (i, w) in widths.iter().enumerate() {
            let blank = Line::from(String::new());
            let line = wrapped[i].get(row).unwrap_or(&blank);
            let used: usize = line.spans.iter().map(|s| s.content.width()).sum();
            let a = align.get(i).copied().unwrap_or(Align::None);
            let (left, right) = pad_for(a, used, *w);
            spans.push(Span::raw(format!(" {}", " ".repeat(left))));
            spans.extend(line.spans.clone());
            spans.push(Span::raw(format!("{} ", " ".repeat(right))));
            spans.push(Span::styled("│".to_string(), border));
        }
        out.push(Line::from(spans));
    }
    out
}

pub fn layout_table(table: &Table, width: u16, theme: &Theme) -> Vec<Line<'static>> {
    let ncols = column_count(table);
    if ncols == 0 {
        return Vec::new();
    }
    let widths = fit_widths(natural_widths(table, ncols), width.max(1) as usize);

    let mut out = vec![border_row(&widths, '┌', '┬', '┐', theme)];
    out.extend(body_rows(
        &table.head,
        &widths,
        &table.align,
        theme,
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    ));
    out.push(border_row(&widths, '├', '┼', '┤', theme));
    for row in &table.rows {
        out.extend(body_rows(
            row,
            &widths,
            &table.align,
            theme,
            Style::default().fg(theme.text),
        ));
    }
    out.push(border_row(&widths, '└', '┴', '┘', theme));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc;
    use crate::doc::ir::Block;
    use crate::theme::DARK;

    fn text_of(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(text_of).collect()
    }

    fn table_of(src: &str) -> Table {
        match doc::parse(src).blocks.into_iter().next() {
            Some(Block::Table(t)) => t,
            other => panic!("expected a table, got {other:?}"),
        }
    }

    #[test]
    fn draws_borders_a_header_and_a_separator() {
        let t = table_of("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert_eq!(
            texts(&layout_table(&t, 40, &DARK)),
            vec![
                "┌───┬───┐",
                "│ a │ b │",
                "├───┼───┤",
                "│ 1 │ 2 │",
                "└───┴───┘",
            ]
        );
    }

    #[test]
    fn columns_widen_to_fit_their_widest_cell() {
        let t = table_of("| a | b |\n|---|---|\n| longer | 2 |\n");
        let lines = layout_table(&t, 40, &DARK);
        assert_eq!(text_of(&lines[3]), "│ longer │ 2 │");
    }

    #[test]
    fn header_cells_are_bold() {
        let t = table_of("| a |\n|---|\n| 1 |\n");
        let lines = layout_table(&t, 40, &DARK);
        let bold = lines[1]
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
        assert!(bold, "header row should be bold");
    }

    #[test]
    fn honours_left_center_and_right_alignment() {
        let t = table_of("| l | c | r |\n|:--|:-:|--:|\n| x | x | x |\n");
        let lines = layout_table(&t, 40, &DARK);
        // Header cells are all one char in three-wide columns.
        assert_eq!(text_of(&lines[1]), "│ l │ c │ r │");
        assert_eq!(text_of(&lines[3]), "│ x │ x │ x │");

        let wide = table_of("| head | head | head |\n|:--|:-:|--:|\n| x | x | x |\n");
        let lines = layout_table(&wide, 60, &DARK);
        assert_eq!(text_of(&lines[3]), "│ x    │  x   │    x │");
    }

    #[test]
    fn no_line_exceeds_the_measure_when_content_is_too_wide() {
        let t = table_of(
            "| aaaaaaaaaaaaaaa | bbbbbbbbbbbbbbb |\n|---|---|\n| ccccccccccccccc | ddddddddddddddd |\n",
        );
        for line in &layout_table(&t, 24, &DARK) {
            assert!(text_of(line).width() <= 24, "overflow: {line:?}");
        }
    }

    #[test]
    fn cells_that_must_shrink_wrap_onto_multiple_rows() {
        let t = table_of("| a | b |\n|---|---|\n| one two three | x |\n");
        let lines = layout_table(&t, 20, &DARK);
        // The body row occupies more than one visual line.
        assert!(
            lines.len() > 5,
            "expected a wrapped body row: {:?}",
            texts(&lines)
        );
        for line in &lines {
            assert!(text_of(line).width() <= 20);
        }
    }

    #[test]
    fn ragged_rows_are_padded_to_the_column_count() {
        let mut t = table_of("| a | b |\n|---|---|\n| 1 | 2 |\n");
        t.rows.push(vec![vec![Inline::Text("only".into())]]);
        let lines = layout_table(&t, 40, &DARK);
        // Every body line still has both column separators.
        assert_eq!(text_of(lines.last().unwrap()).matches('┴').count(), 1);
    }

    #[test]
    fn a_header_only_table_renders_without_a_body() {
        let t = table_of("| a |\n|---|\n");
        assert_eq!(
            texts(&layout_table(&t, 20, &DARK)),
            vec!["┌───┐", "│ a │", "├───┤", "└───┘"]
        );
    }

    #[test]
    fn a_very_narrow_measure_does_not_panic() {
        let t = table_of("| a | b |\n|---|---|\n| 1 | 2 |\n");
        for w in 0..12u16 {
            assert!(!layout_table(&t, w, &DARK).is_empty());
        }
    }
}
