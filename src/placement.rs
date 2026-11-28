//! Where the overlay sits on its monitor and how big it is: margins from the monitor's top edge
//! and from the side it's on, as `config.toml` keeps them. QML draws and moves the overlay by its
//! top-left corner, in pixels from the monitor's; these turn one into the other and keep the
//! overlay on the monitor.

use crate::config::Side;

/// How much of the overlay stays on the monitor when it's moved down, so its header can be grabbed.
const MIN_VISIBLE_HEIGHT: i32 = 120;
/// The overlay's smallest size when resized with its edges.
const MIN_WIDTH: i32 = 300;
const MIN_HEIGHT: i32 = 150;
/// Room left below the overlay, so it never reaches the monitor's bottom edge.
const BOTTOM_GAP: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margins {
    pub top: i32,
    pub side: i32,
}

/// A monitor's size in pixels.
#[derive(Debug, Clone, Copy)]
pub struct ScreenSize {
    pub width: i32,
    pub height: i32,
}

/// The left edge of an overlay `width` pixels wide whose side margin is `margin`.
pub fn left_edge(margin: i32, side: Side, screen: ScreenSize, width: i32) -> i32 {
    match side {
        Side::Left => margin,
        Side::Right => screen.width - width - margin,
    }
}

/// The margins that put an overlay `width` pixels wide closest to `(x, y)` while keeping it on
/// the monitor: all of its width, and enough of its height to grab the header.
pub fn margins_at(x: f64, y: f64, side: Side, screen: ScreenSize, width: i32) -> Margins {
    let x = clamp(x, 0, screen.width - width);
    // Measured from the same side, so `left_edge` turns the margin back into `x`.
    let side_margin = left_edge(x, side, screen, width);
    Margins { top: clamp(y, 0, screen.height - MIN_VISIBLE_HEIGHT), side: side_margin.max(0) }
}

/// The width for dragging the overlay's free side edge `dx` pixels, from `width` with its left
/// edge at `x`. That's the edge away from its side, so a right-hand overlay grows to the left.
pub fn dragged_width(x: f64, width: f64, dx: f64, side: Side, screen: ScreenSize) -> i32 {
    let (wanted, room) = match side {
        Side::Left => (width + dx, screen.width as f64 - x),
        Side::Right => (width - dx, x + width),
    };
    let room = room.round() as i32;
    clamp(wanted, MIN_WIDTH.min(room), room)
}

/// The height for dragging the overlay's bottom edge `dy` pixels, from `height` with its top at `y`.
pub fn dragged_height(y: f64, height: f64, dy: f64, screen: ScreenSize) -> i32 {
    let room = height_limit(y.round() as i32, screen);
    clamp(height + dy, MIN_HEIGHT.min(room), room)
}

/// The tallest the overlay may be with its top `top` pixels down the monitor.
pub fn height_limit(top: i32, screen: ScreenSize) -> i32 {
    screen.height - top - BOTTOM_GAP
}

/// `value` rounded into `min..=max`; `min` wins when the range is empty (nothing fits).
fn clamp(value: f64, min: i32, max: i32) -> i32 {
    (value.round() as i32).min(max).max(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: ScreenSize = ScreenSize { width: 2560, height: 1440 };
    const MARGINS: Margins = Margins { top: 80, side: 24 };

    #[test]
    fn left_edge_is_measured_from_the_overlays_side() {
        assert_eq!(left_edge(24, Side::Left, SCREEN, 380), 24);
        assert_eq!(left_edge(24, Side::Right, SCREEN, 380), 2156);
    }

    #[test]
    fn margins_at_undoes_left_edge() {
        for side in [Side::Left, Side::Right] {
            let x = left_edge(MARGINS.side, side, SCREEN, 380);
            assert_eq!(margins_at(x.into(), MARGINS.top.into(), side, SCREEN, 380), MARGINS);
        }
    }

    #[test]
    fn right_side_margin_shrinks_towards_the_right_edge() {
        assert_eq!(margins_at(2170.0, 60.0, Side::Right, SCREEN, 380), Margins { top: 60, side: 10 });
    }

    #[test]
    fn rounds_fractional_positions() {
        assert_eq!(margins_at(24.4, 80.6, Side::Left, SCREEN, 380), Margins { top: 81, side: 24 });
    }

    #[test]
    fn keeps_the_overlay_on_the_monitor() {
        assert_eq!(margins_at(-50.0, -50.0, Side::Left, SCREEN, 380), Margins { top: 0, side: 0 });
        assert_eq!(margins_at(5000.0, 5000.0, Side::Left, SCREEN, 380), Margins { top: 1320, side: 2180 });
        assert_eq!(margins_at(5000.0, 0.0, Side::Right, SCREEN, 380), Margins { top: 0, side: 0 });
        assert_eq!(margins_at(-50.0, 0.0, Side::Right, SCREEN, 380), Margins { top: 0, side: 2180 });
    }

    #[test]
    fn overlay_wider_than_the_monitor_sits_at_its_side() {
        assert_eq!(margins_at(100.0, 80.0, Side::Left, SCREEN, 3000), Margins { top: 80, side: 0 });
        assert_eq!(margins_at(100.0, 80.0, Side::Right, SCREEN, 3000), Margins { top: 80, side: 0 });
    }

    #[test]
    fn free_edge_grows_away_from_the_side() {
        assert_eq!(dragged_width(24.0, 380.0, 50.0, Side::Left, SCREEN), 430);
        assert_eq!(dragged_width(2156.0, 380.0, -50.0, Side::Right, SCREEN), 430);
        assert_eq!(dragged_width(2156.0, 380.0, 50.0, Side::Right, SCREEN), 330);
    }

    #[test]
    fn width_stays_between_the_minimum_and_the_room_left() {
        assert_eq!(dragged_width(24.0, 380.0, -500.0, Side::Left, SCREEN), 300);
        assert_eq!(dragged_width(24.0, 380.0, 5000.0, Side::Left, SCREEN), 2536);
        assert_eq!(dragged_width(2156.0, 380.0, -5000.0, Side::Right, SCREEN), 2536);
    }

    #[test]
    fn width_never_overflows_when_less_than_the_minimum_fits() {
        assert_eq!(dragged_width(2400.0, 160.0, 50.0, Side::Left, SCREEN), 160);
    }

    #[test]
    fn height_stays_above_the_bottom_gap() {
        assert_eq!(dragged_height(80.0, 400.0, 100.0, SCREEN), 500);
        assert_eq!(dragged_height(80.0, 400.0, -1000.0, SCREEN), 150);
        assert_eq!(dragged_height(80.0, 400.0, 5000.0, SCREEN), 1320);
        assert_eq!(height_limit(80, SCREEN), 1320);
    }

    #[test]
    fn height_never_overflows_near_the_bottom() {
        assert_eq!(dragged_height(1320.0, 80.0, 50.0, SCREEN), 80);
    }
}
