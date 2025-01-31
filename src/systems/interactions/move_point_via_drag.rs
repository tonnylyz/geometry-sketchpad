use specs::prelude::*;
use crate::{
  resources::{
    Tool,
    InputState,
    Viewport, ViewportTransform,
    SpatialHashTable,
    events::{
      ToolChangeEvent, ToolChangeEventChannel, ToolChangeEventReader,
      SketchEventChannel, SketchEvent, MovePoint,
      MouseEvent, MouseEventChannel, MouseEventReader,
    },
  },
  components::{SymbolicPoint, Point, Line},
};
use super::helpers::hitting_object;

static SELECT_DIST_THRES : f64 = 5.0; // Pixel

pub struct MovePointViaDrag {
  tool_change_event_reader: Option<ToolChangeEventReader>,
  mouse_event_reader: Option<MouseEventReader>,
  dragging_point: Option<(Entity, SymbolicPoint)>,
}

impl Default for MovePointViaDrag {
  fn default() -> Self {
    Self {
      tool_change_event_reader: None,
      mouse_event_reader: None,
      dragging_point: None,
    }
  }
}

impl<'a> System<'a> for MovePointViaDrag {
  type SystemData = (
    Read<'a, InputState>,
    Read<'a, ToolChangeEventChannel>,
    Write<'a, MouseEventChannel>,
    Read<'a, Viewport>,
    Read<'a, SpatialHashTable<Entity>>,
    Write<'a, SketchEventChannel>,
    ReadStorage<'a, SymbolicPoint>,
    ReadStorage<'a, Point>,
    ReadStorage<'a, Line>,
  );

  fn setup(&mut self, world: &mut World) {
    Self::SystemData::setup(world);
    self.tool_change_event_reader = Some(world.fetch_mut::<ToolChangeEventChannel>().register_reader());
    self.mouse_event_reader = Some(world.fetch_mut::<MouseEventChannel>().register_reader());
  }

  fn run(&mut self, (
    input_state,
    tool_change_event_channel,
    mut mouse_event_channel,
    viewport,
    spatial_table,
    mut sketch_event_channel,
    sym_points,
    points,
    lines,
  ): Self::SystemData) {

    // First use tool change to setup mouse event reader.
    // We will only listen to mouse event when the tool state is select.
    // We will drop the mouse event listener when the tool state is set to others.
    if let Some(reader_id) = &mut self.tool_change_event_reader {
      for event in tool_change_event_channel.read(reader_id) {
        match event {
          ToolChangeEvent(Tool::Select) => {
            self.mouse_event_reader = Some(mouse_event_channel.register_reader());
          },
          _ => {
            if let Some(reader_id) = &mut self.mouse_event_reader {
              drop(reader_id);
              self.mouse_event_reader = None;
            }
          }
        }
      }
    }

    if let Some(reader_id) = &mut self.mouse_event_reader {
      for event in mouse_event_channel.read(reader_id) {
        match event {
          MouseEvent::DragBegin(start_position) => {
            if !input_state.keyboard.is_shift_activated() {
              if let Some(entity) = hitting_object(*start_position, &viewport, &spatial_table, &points, &lines, SELECT_DIST_THRES) {
                if let Some(sym_point) = sym_points.get(entity) {
                  self.dragging_point = Some((entity, *sym_point));

                  // Note that we let the dragging point to be selected directly
                  sketch_event_channel.single_write(SketchEvent::Select(entity));
                }
              }
            }
          },
          MouseEvent::DragMove(_, curr_position) => {
            match self.dragging_point {
              Some((ent, sym_point)) => {
                match sym_point {
                  SymbolicPoint::Free(old_position) => {
                    let new_position = curr_position.to_virtual(&viewport);
                    sketch_event_channel.single_write(SketchEvent::MovePoint(ent, MovePoint::Free(old_position, new_position)));
                  },
                  SymbolicPoint::OnLine(line_entity, old_t) => {
                    if let Some(line) = lines.get(line_entity) {
                      let virtual_mouse_position = curr_position.to_virtual(&viewport);
                      let projected_position = virtual_mouse_position.project(*line);
                      let diff = projected_position - line.origin;
                      let sign = diff.dot(line.direction).signum();
                      let new_t = sign * diff.magnitude();
                      sketch_event_channel.single_write(SketchEvent::MovePoint(ent, MovePoint::OnLine(line_entity, old_t, new_t)))
                    }
                  },
                  _ => (),
                }
              },
              None => (),
            }
          },
          MouseEvent::DragEnd(_) => {
            self.dragging_point = None;
          },
          _ => (),
        }
      }
    }
  }
}