//! Data-driven card sizing, shared by the canvas and game renderer.
use crate::*;

/// Preserve the grid's order and minimum cell size while accommodating tall cards.
pub fn fit_card_rows(bounds: &mut [[f32; 4]], heights: &[f32], gap: f32) {
    let original_y: Vec<f32> = bounds.iter().map(|r| r[1]).collect();
    let mut rows = original_y.clone();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 0.1);
    let mut top = rows.first().copied().unwrap_or(0.0);
    for y in rows {
        let indices: Vec<_> = original_y
            .iter()
            .enumerate()
            .filter(|(_, value)| (**value - y).abs() < 0.1)
            .map(|(i, _)| i)
            .collect();
        let height = indices
            .iter()
            .map(|i| bounds[*i][3].max(heights.get(*i).copied().unwrap_or(0.0)))
            .fold(0.0, f32::max);
        for i in indices {
            bounds[i][1] = top;
            bounds[i][3] = height;
        }
        top += height + gap;
    }
}

/// Expand opted-in fields in free-layout cards without overlapping later fields.
pub(crate) fn fit_card_fields(original: &[Element], items: &mut [Element]) {
    if !items.iter().any(|e| e.layout_options.auto_height) {
        return;
    }
    let mut order: Vec<_> = (1..items.len()).collect();
    order.sort_by(|a, b| original[*a].rect[1].total_cmp(&original[*b].rect[1]));
    for (at, &i) in order.iter().enumerate() {
        let old = original[i].rect;
        let current = items[i].rect;
        let increase = (current[3] - old[3]).max(0.0);
        if increase < 0.1 {
            continue;
        }
        for &j in &order[at + 1..] {
            let other = original[j].rect;
            if other[1] >= old[1] + old[3] - 0.1
                && other[0] < old[0] + old[2]
                && other[0] + other[2] > old[0]
            {
                // Automatic containers may already have advanced this sibling.
                items[j].rect[1] =
                    items[j].rect[1].max(current[1] + current[3] + other[1] - old[1] - old[3]);
            }
        }
    }
    let bottom = items
        .iter()
        .skip(1)
        .map(|e| e.rect[1] + e.rect[3])
        .fold(0.0, f32::max);
    if let Some(root) = items.first_mut() {
        root.rect[3] = root.rect[3].max(bottom + 4.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_keep_column_order_and_make_room_for_long_cards() {
        let mut bounds = vec![
            [0.0, 0.0, 100.0, 100.0],
            [0.0, 110.0, 100.0, 100.0],
            [110.0, 0.0, 100.0, 100.0],
            [110.0, 110.0, 100.0, 100.0],
        ];
        fit_card_rows(&mut bounds, &[180.0, 100.0, 100.0, 120.0], 10.0);
        assert_eq!(bounds[0], [0.0, 0.0, 100.0, 180.0]);
        assert_eq!(bounds[1], [0.0, 190.0, 100.0, 120.0]);
        assert_eq!(bounds[2], [110.0, 0.0, 100.0, 180.0]);
    }
}
