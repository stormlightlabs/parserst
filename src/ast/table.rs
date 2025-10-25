use super::parse_inlines;
use crate::{Block, Inline, Lines};

/// Check if a line is a simple table separator (all = and spaces)
fn is_table_separator(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.chars().all(|c| c == '=' || c == ' ')
}

/// Check if a line is a grid table border (+--+--+)
fn is_grid_border(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() || !trimmed.starts_with('+') {
        return false;
    }
    trimmed.chars().all(|c| c == '+' || c == '-' || c == '=' || c == ' ')
}

/// Parse column positions from a grid table border line
fn parse_grid_columns(border: &str) -> Vec<usize> {
    border
        .char_indices()
        .filter_map(|(i, c)| if c == '+' { Some(i) } else { None })
        .collect()
}

/// Check if a grid border line is a header separator (contains =)
fn is_grid_header_separator(s: &str) -> bool {
    s.contains('=')
}

/// Parse column boundaries from a separator line
fn parse_column_boundaries(sep: &str) -> Vec<(usize, usize)> {
    let mut columns = Vec::new();
    let mut in_col = false;
    let mut start = 0;

    for (i, ch) in sep.char_indices() {
        if ch == '=' {
            if !in_col {
                in_col = true;
                start = i;
            }
        } else if in_col {
            columns.push((start, i));
            in_col = false;
        }
    }
    if in_col {
        columns.push((start, sep.len()));
    }
    columns
}

/// Extract cell content from a line based on column boundaries
fn extract_cells(line: &str, columns: &[(usize, usize)]) -> Vec<String> {
    columns
        .iter()
        .map(|(start, end)| {
            let cell_text = if *start < line.len() {
                let end_bounded = (*end).min(line.len());
                &line[*start..end_bounded]
            } else {
                ""
            };
            cell_text.trim().to_string()
        })
        .collect()
}

/// Try to parse a simple table (=== separators)
pub fn try_parse_simple_table(ls: &mut Lines<'_>) -> Option<Block> {
    let first_line = ls.peek()?;
    if !is_table_separator(first_line.raw) {
        return None;
    }

    let separator = first_line.raw;
    let columns = parse_column_boundaries(separator);
    if columns.is_empty() {
        return None;
    }

    ls.next();

    let header_line = ls.peek()?;
    if is_table_separator(header_line.raw) {
        ls.backtrack();
        return None;
    }
    let header_cells = extract_cells(header_line.raw, &columns);
    ls.next();

    if !ls.peek().map(|l| is_table_separator(l.raw)).unwrap_or(false) {
        ls.backtrack();
        ls.backtrack();
        return None;
    }
    ls.next();

    let mut body_rows = Vec::new();
    while let Some(line) = ls.peek() {
        if is_table_separator(line.raw) {
            ls.next();
            break;
        }
        let cells = extract_cells(line.raw, &columns);
        body_rows.push(cells);
        ls.next();
    }

    let headers: Vec<Vec<Inline>> = header_cells.into_iter().map(|cell| parse_inlines(&cell)).collect();

    let rows: Vec<Vec<Vec<Inline>>> = body_rows
        .into_iter()
        .map(|row| row.into_iter().map(|cell| parse_inlines(&cell)).collect())
        .collect();

    Some(Block::Table { headers, rows })
}

/// Extract grid table cell from a row based on column positions
fn extract_grid_cell(row: &str, start_col: usize, end_col: usize) -> String {
    if start_col >= row.len() {
        return String::new();
    }
    let end = end_col.min(row.len());
    let cell_text = &row[start_col..end];

    cell_text
        .trim_matches(|c: char| c == '|' || c.is_whitespace())
        .to_string()
}

/// Try to parse a grid table (+---+---+)
pub fn try_parse_grid_table(ls: &mut Lines<'_>) -> Option<Block> {
    let first_border = ls.peek()?;
    if !is_grid_border(first_border.raw) {
        return None;
    }

    let col_positions = parse_grid_columns(first_border.raw);
    if col_positions.len() < 2 {
        return None;
    }

    ls.next();

    let mut all_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row_lines: Vec<String> = Vec::new();
    let mut header_row_count = 0;
    let mut found_header_sep = false;

    while let Some(line) = ls.peek() {
        if is_grid_border(line.raw) {
            if !current_row_lines.is_empty() {
                let merged_row = merge_multi_line_row(&current_row_lines, &col_positions);
                all_rows.push(merged_row);
                current_row_lines.clear();

                if !found_header_sep {
                    header_row_count = all_rows.len();
                }
            }

            if is_grid_header_separator(line.raw) && !found_header_sep {
                found_header_sep = true;
            }

            ls.next();

            if let Some(next) = ls.peek() {
                if !is_grid_border(next.raw) && !next.raw.trim_start().starts_with('|') {
                    break;
                }
            } else {
                break;
            }
        } else if line.raw.trim_start().starts_with('|') {
            current_row_lines.push(line.raw.to_string());
            ls.next();
        } else {
            break;
        }
    }

    if !current_row_lines.is_empty() {
        let merged_row = merge_multi_line_row(&current_row_lines, &col_positions);
        all_rows.push(merged_row);
    }

    if all_rows.is_empty() {
        return None;
    }

    let (header_rows, body_rows) = if header_row_count > 0 {
        all_rows.split_at(header_row_count)
    } else {
        (&all_rows[..0], all_rows.as_slice())
    };

    let headers: Vec<Vec<Inline>> = if !header_rows.is_empty() {
        header_rows[0].iter().map(|cell| parse_inlines(cell)).collect()
    } else {
        Vec::new()
    };

    let rows: Vec<Vec<Vec<Inline>>> = body_rows
        .iter()
        .map(|row| row.iter().map(|cell| parse_inlines(cell)).collect())
        .collect();

    Some(Block::Table { headers, rows })
}

/// Merge multiple lines of a grid table row into single cells
fn merge_multi_line_row(lines: &[String], col_positions: &[usize]) -> Vec<String> {
    let num_cols = col_positions.len().saturating_sub(1);
    let mut cells: Vec<String> = vec![String::new(); num_cols];

    for line in lines {
        for col_idx in 0..num_cols {
            let start = col_positions[col_idx];
            let end = col_positions[col_idx + 1];
            let cell_content = extract_grid_cell(line, start, end);

            if !cell_content.is_empty() {
                if !cells[col_idx].is_empty() {
                    cells[col_idx].push(' ');
                }
                cells[col_idx].push_str(&cell_content);
            }
        }
    }

    cells
}
