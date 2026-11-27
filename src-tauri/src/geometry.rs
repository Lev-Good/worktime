//! Screen geometry for placing the floating clock window.

/// A rectangle in logical pixels: top-left corner plus size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    /// Converts a monitor rectangle reported in physical pixels to logical pixels.
    pub fn from_physical(x: i32, y: i32, width: u32, height: u32, scale: f64) -> Rect {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        Rect {
            x: x as f64 / scale,
            y: y as f64 / scale,
            width: width as f64 / scale,
            height: height as f64 / scale,
        }
    }

    fn overlap(&self, other: &Rect) -> (f64, f64) {
        let width = (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
        let height = (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
        (width.max(0.0), height.max(0.0))
    }
}

/// True when at least `min_width` x `min_height` of `window` lands on one of `monitors`,
/// i.e. the window is still visible and the user can grab it again.
pub fn is_reachable(window: Rect, monitors: &[Rect], min_width: f64, min_height: f64) -> bool {
    monitors.iter().any(|monitor| {
        let (width, height) = window.overlap(monitor);
        width >= min_width && height >= min_height
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn window(x: f64, y: f64) -> Rect {
        Rect {
            x,
            y,
            width: 252.0,
            height: 92.0,
        }
    }

    #[test]
    fn a_window_inside_a_monitor_is_reachable() {
        let monitors = [monitor(0.0, 0.0, 1920.0, 1080.0)];
        assert!(is_reachable(window(28.0, 892.0), &monitors, 80.0, 40.0));
    }

    #[test]
    fn a_window_hanging_off_the_edge_is_kept_while_enough_of_it_shows() {
        let monitors = [monitor(0.0, 0.0, 1920.0, 1080.0)];
        // 80 of the 252 logical pixels of width are still on the screen.
        assert!(is_reachable(window(1840.0, 500.0), &monitors, 80.0, 40.0));
        // Only 20 are left: not enough to grab the window again.
        assert!(!is_reachable(window(1900.0, 500.0), &monitors, 80.0, 40.0));
    }

    #[test]
    fn a_position_saved_on_a_monitor_that_is_gone_is_not_reachable() {
        let two_screens = [
            monitor(0.0, 0.0, 1920.0, 1080.0),
            monitor(1920.0, 0.0, 1920.0, 1080.0),
        ];
        let only_primary = [monitor(0.0, 0.0, 1920.0, 1080.0)];
        assert!(is_reachable(window(2000.0, 300.0), &two_screens, 80.0, 40.0));
        assert!(!is_reachable(window(2000.0, 300.0), &only_primary, 80.0, 40.0));
    }

    #[test]
    fn a_monitor_to_the_left_of_the_origin_counts() {
        let monitors = [
            monitor(-1920.0, 0.0, 1920.0, 1080.0),
            monitor(0.0, 0.0, 1920.0, 1080.0),
        ];
        assert!(is_reachable(window(-1000.0, 200.0), &monitors, 80.0, 40.0));
    }

    #[test]
    fn no_monitors_means_not_reachable() {
        assert!(!is_reachable(window(0.0, 0.0), &[], 80.0, 40.0));
    }

    #[test]
    fn physical_monitor_rects_are_scaled_to_logical_pixels() {
        let rect = Rect::from_physical(0, 0, 2560, 1440, 1.25);
        assert_eq!(rect, monitor(0.0, 0.0, 2048.0, 1152.0));
        // Saved in logical pixels near the bottom-right corner of that screen.
        assert!(is_reachable(window(1900.0, 1000.0), &[rect], 80.0, 40.0));
        // A physical-pixel position from a wider screen would fall outside it.
        assert!(!is_reachable(window(2050.0, 1000.0), &[rect], 80.0, 40.0));
    }
}
