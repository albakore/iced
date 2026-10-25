//! Compute the damage between frames.
use crate::core::{Point, Rectangle};

/// Diffs the damage regions given some previous and current primitives.
///
/// pyzed: when primitives were added or removed (a hover background, a tooltip layer), the
/// ones after them are paired again past the gap, instead of each one with its neighbor, so a
/// new item damages only itself and not everything drawn after it.
pub fn diff<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    diff: impl Fn(&T, &T) -> Vec<Rectangle>,
) -> Vec<Rectangle> {
    if previous.len() == current.len() {
        return previous
            .iter()
            .zip(current)
            .flat_map(|(a, b)| diff(a, b))
            .collect();
    }

    let mut damage = Vec::new();
    let (mut i, mut j) = (0, 0);

    while i < previous.len() && j < current.len() {
        let (a, b) = (&previous[i], &current[j]);
        let changes = diff(a, b);

        if changes.is_empty() {
            i += 1;
            j += 1;
        } else if current.len() - j > previous.len() - i
            && diff(a, &current[j + 1]).is_empty()
        {
            // `b` is new
            damage.extend(bounds(b));
            i += 1;
            j += 2;
        } else if previous.len() - i > current.len() - j
            && diff(&previous[i + 1], b).is_empty()
        {
            // `a` is gone
            damage.extend(bounds(a));
            i += 2;
            j += 1;
        } else {
            damage.extend(changes);
            i += 1;
            j += 1;
        }
    }

    // Extend damage by the added/removed primitives
    damage.extend(previous[i..].iter().flat_map(&bounds));
    damage.extend(current[j..].iter().flat_map(&bounds));
    damage
}

/// Computes the damage regions given some previous and current primitives.
pub fn list<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    are_equal: impl Fn(&T, &T) -> bool,
) -> Vec<Rectangle> {
    diff(previous, current, &bounds, |a, b| {
        if are_equal(a, b) {
            vec![]
        } else {
            bounds(a).into_iter().chain(bounds(b)).collect()
        }
    })
}

/// Groups the given damage regions that are close together inside the given
/// bounds.
pub fn group(mut damage: Vec<Rectangle>, bounds: Rectangle) -> Vec<Rectangle> {
    const AREA_THRESHOLD: f32 = 20_000.0;

    damage.sort_by(|a, b| {
        a.center()
            .distance_squared(Point::ORIGIN)
            .total_cmp(&b.center().distance_squared(Point::ORIGIN))
    });

    let mut output = Vec::new();
    let mut scaled = damage
        .into_iter()
        .filter_map(|region| region.intersection(&bounds))
        .filter(|region| region.width >= 1.0 && region.height >= 1.0);

    if let Some(mut current) = scaled.next() {
        for region in scaled {
            let union = current.union(&region);

            if union.area() - current.area() - region.area() <= AREA_THRESHOLD {
                current = union;
            } else {
                output.push(current);
                current = region;
            }
        }

        output.push(current);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(y: f32) -> Rectangle {
        Rectangle::new(Point::new(0.0, y), crate::core::Size::new(10.0, 10.0))
    }

    fn damage(previous: &[f32], current: &[f32]) -> Vec<Rectangle> {
        list(
            &previous.iter().copied().map(rect).collect::<Vec<_>>(),
            &current.iter().copied().map(rect).collect::<Vec<_>>(),
            |bounds| vec![*bounds],
            |a, b| a == b,
        )
    }

    #[test]
    fn an_added_item_damages_only_itself() {
        assert_eq!(
            damage(&[0.0, 20.0, 40.0], &[0.0, 5.0, 20.0, 40.0]),
            [rect(5.0)]
        );
        assert_eq!(
            damage(&[0.0, 20.0, 40.0], &[0.0, 20.0, 40.0, 60.0]),
            [rect(60.0)]
        );
    }

    #[test]
    fn a_removed_item_damages_only_itself() {
        assert_eq!(
            damage(&[0.0, 5.0, 20.0, 40.0], &[0.0, 20.0, 40.0]),
            [rect(5.0)]
        );
    }

    #[test]
    fn changed_items_still_damage_both_places() {
        assert_eq!(
            damage(&[0.0, 20.0, 40.0], &[0.0, 25.0, 30.0, 40.0]),
            [rect(20.0), rect(25.0), rect(30.0)]
        );
        assert_eq!(
            damage(&[0.0, 20.0], &[0.0, 30.0]),
            [rect(20.0), rect(30.0)]
        );
    }
}
