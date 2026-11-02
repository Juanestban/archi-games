//! Line icons painted with vectors (Lucide-like, 1.5 px strokes), so the app ships no image
//! assets. Each one fills the square `rect` it is given.

use std::f32::consts::{PI, TAU};

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
  User,
  Gamepad,
  Trophy,
  Clock,
  Gear,
  ChevronRight,
  Lock,
}

/// Points along a circle from angle `a0` to `a1` (radians, 0 = right, clockwise on screen).
pub fn arc(center: Pos2, radius: f32, a0: f32, a1: f32, steps: usize) -> Vec<Pos2> {
  (0..=steps)
    .map(|i| {
      let a = a0 + (a1 - a0) * i as f32 / steps as f32;
      center + radius * vec2(a.cos(), a.sin())
    })
    .collect()
}

pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
  let s = rect.width() / 24.0; // Designed on a 24 px grid.
  let at = |x: f32, y: f32| rect.min + vec2(x * s, y * s);
  let stroke = Stroke::new(1.5 * s.max(0.75), color);
  let line = |points: Vec<Pos2>| p.add(Shape::line(points, stroke));

  match icon {
    Icon::User => {
      p.circle_stroke(at(12.0, 8.0), 4.0 * s, stroke);
      line(arc(at(12.0, 22.0), 8.0 * s, PI, TAU, 16));
    }
    Icon::Gamepad => {
      let body = Rect::from_min_max(at(2.0, 7.0), at(22.0, 18.0));
      p.rect_stroke(body, 5.0 * s, stroke, eframe::egui::StrokeKind::Middle);
      line(vec![at(6.0, 12.5), at(10.0, 12.5)]);
      line(vec![at(8.0, 10.5), at(8.0, 14.5)]);
      p.circle_filled(at(15.5, 11.5), 1.1 * s, color);
      p.circle_filled(at(18.0, 14.0), 1.1 * s, color);
    }
    Icon::Trophy => trophy(p, rect, color, false),
    Icon::Clock => {
      p.circle_stroke(at(12.0, 12.0), 9.0 * s, stroke);
      line(vec![at(12.0, 7.0), at(12.0, 12.0), at(15.5, 14.0)]);
    }
    Icon::Gear => {
      let c = at(12.0, 12.0);
      p.circle_stroke(c, 3.0 * s, stroke);
      let teeth: Vec<Pos2> = (0..48)
        .map(|i| {
          let a = i as f32 / 48.0 * TAU;
          let r = if (i / 3) % 2 == 0 { 9.0 } else { 7.0 };
          c + r * s * vec2(a.cos(), a.sin())
        })
        .collect();
      p.add(Shape::closed_line(teeth, stroke));
    }
    Icon::ChevronRight => {
      line(vec![at(9.0, 6.0), at(15.0, 12.0), at(9.0, 18.0)]);
    }
    Icon::Lock => {
      p.rect_stroke(Rect::from_min_max(at(5.0, 11.0), at(19.0, 21.0)), 2.0 * s, stroke, eframe::egui::StrokeKind::Middle);
      line(arc(at(12.0, 11.0), 4.0 * s, PI, TAU, 12).into_iter().chain([at(16.0, 11.0)]).collect());
      p.circle_filled(at(12.0, 16.0), 1.2 * s, color);
    }
  }
}

/// A trophy cup: outlined as an icon, or `solid` for badges.
pub fn trophy(p: &Painter, rect: Rect, color: Color32, solid: bool) {
  let s = rect.width() / 24.0;
  let at = |x: f32, y: f32| rect.min + vec2(x * s, y * s);
  let stroke = Stroke::new(1.5 * s.max(0.75), color);
  // Bowl: a flat rim narrowing into a rounded bottom.
  let mut bowl = vec![at(6.0, 3.0), at(18.0, 3.0)];
  bowl.extend(arc(at(12.0, 9.0), 6.0 * s, 0.0, PI, 16));
  let handles = [arc(at(6.0, 7.0), 3.0 * s, PI * 0.5, PI * 1.5, 10), arc(at(18.0, 7.0), 3.0 * s, -PI * 0.5, PI * 0.5, 10)];
  let stem = [at(12.0, 15.0), at(12.0, 18.5)];
  let base = Rect::from_min_max(at(7.5, 18.5), at(16.5, 21.0));
  if solid {
    p.add(Shape::convex_polygon(bowl, color, Stroke::NONE));
    for handle in handles {
      p.add(Shape::line(handle, Stroke::new(2.2 * s, color)));
    }
    p.add(Shape::line_segment(stem, Stroke::new(2.6 * s, color)));
    p.rect_filled(base, 1.0 * s, color);
  } else {
    p.add(Shape::closed_line(bowl, stroke));
    for handle in handles {
      p.add(Shape::line(handle, stroke));
    }
    p.add(Shape::line_segment(stem, stroke));
    p.rect_stroke(base, 1.0 * s, stroke, eframe::egui::StrokeKind::Middle);
  }
}

/// A square of `size` centered on `center`, for [`paint`].
pub fn square(center: Pos2, size: f32) -> Rect {
  Rect::from_center_size(center, vec2(size, size))
}
