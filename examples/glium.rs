/* Generic tree structures for storage of spatial data.
 * Copyright (C) 2023  Alexander Pyattaev
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use glium::glutin::event_loop::EventLoop;
use glium::index::PrimitiveType;
use glium::{
    glutin, implement_vertex, program, uniform, Display, IndexBuffer, Program, Surface,
    VertexBuffer,
};

use spatialtree::coords::QuadVec;
use spatialtree::*;

// the chunk struct for the tree
#[allow(dead_code)]
struct Chunk {
    visible: bool,
    cache_state: i32,
    // 0 is new, 1 is merged, 2 is cached, 3 is both
    selected: bool,
    in_bounds: bool,
}

fn make_shaders(display: &Display) -> Program {
    let program = program!(display,
    140 => {
        vertex: "
			#version 140

			uniform vec2 offset;
			uniform float scale;

			in vec2 position;

			void main() {

				vec2 local_position = position * scale + 0.005;
				local_position.x = min(local_position.x, scale) - 0.0025;
				local_position.y = min(local_position.y, scale) - 0.0025;

				gl_Position = vec4(local_position + (offset + scale * 0.5) * 2.0 - 1.0, 0.0, 1.0);
			}
		",

        fragment: "
			#version 140

			uniform int state;
			uniform int selected;
			uniform int in_bounds;

			void main() {

				if (state == 0) gl_FragColor = vec4(0.2, 0.2, 0.2, 1.0); // new, white
				if (state == 1) gl_FragColor = vec4(0.0, 0.2, 0.0, 1.0); // merged, green
				if (state == 2) gl_FragColor = vec4(0.2, 0.0, 0.0, 1.0); // from cache, red
				if (state == 3) gl_FragColor = vec4(0.4, 0.2, 0.0, 1.0); // both, yellow

				if (selected != 0) gl_FragColor = vec4(0.0, 0.2, 0.2, 1.0); // selected, so blue
				if (in_bounds != 0) gl_FragColor = vec4(0.0, 0.2, 0.2, 1.0); // in bounds, so purple

			}
		"
    }
    )
    .unwrap();
    return program;
}

#[derive(Copy, Clone)]
struct Vertex {
    // only need a 2d position
    position: [f32; 2],
}
implement_vertex!(Vertex, position);

struct RenderContext {
    display: Display,
    vertex_buffer: VertexBuffer<Vertex>,
    shaders: Program,
    index_buffer: IndexBuffer<u16>,
}

impl RenderContext {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        let wb = glutin::window::WindowBuilder::new().with_title("Quadtree demo");
        let cb = glutin::ContextBuilder::new().with_vsync(true);
        let display = Display::new(wb, cb, &event_loop).unwrap();
        // make a vertex buffer
        // we'll reuse it as we only need to draw one quad multiple times anyway
        let vertex_buffer = {
            VertexBuffer::new(
                &display,
                &[
                    Vertex {
                        position: [-1.0, -1.0],
                    },
                    Vertex {
                        position: [-1.0, 1.0],
                    },
                    Vertex {
                        position: [1.0, -1.0],
                    },
                    Vertex {
                        position: [1.0, 1.0],
                    },
                ],
            )
            .unwrap()
        };
        // and the index buffer to form the triangle
        let index_buffer = IndexBuffer::new(
            &display,
            PrimitiveType::TrianglesList,
            &[0 as u16, 1, 2, 1, 2, 3],
        )
        .unwrap();

        let shaders = make_shaders(&display);
        Self {
            display,
            vertex_buffer,
            index_buffer,
            shaders,
        }
    }
}

fn draw(mouse_pos: (f32, f32), tree: &mut QuadTree<Chunk, QuadVec>, ctx: &RenderContext) {
    //function for adding chunks to their respective position, and also set their properties
    fn chunk_creator(_position: QuadVec) -> Chunk {
        Chunk {
            visible: true,
            cache_state: 0,
            selected: false,
            in_bounds: false,
        }
    }

    let qv = QuadVec::from_float_coords([mouse_pos.0, (1.0 - mouse_pos.1)], 6);
    tree.lod_update(&[qv], 2, chunk_creator, |_, _| {});
    // make sure there are no holes in chunks array for fast iteration
    tree.defragment_chunks();
    // and select the chunk at the mouse position
    if let Some(chunk) = tree.get_chunk_from_position_mut(qv) {
        chunk.selected = true;
    }

    // and select a number of chunks in a region when the mouse buttons are selected

    // and, Redraw!
    let mut target = ctx.display.draw();
    target.clear_color(0.6, 0.6, 0.6, 1.0);

    // go over all chunks, iterator version
    for (_, container) in tree.iter_chunks() {
        let (chunk, position) = (&container.chunk, container.position);
        if chunk.visible {
            // draw it if it's visible
            // here we get the chunk position and size
            let uniforms = uniform! {
                offset: position.float_coords(),
                scale: position.float_size() as f32,
                state: chunk.cache_state,
                selected: chunk.selected as i32,
            };

            // draw it with glium
            target
                .draw(
                    &ctx.vertex_buffer,
                    &ctx.index_buffer,
                    &ctx.shaders,
                    &uniforms,
                    &Default::default(),
                )
                .unwrap();
        }
    }
    target.finish().unwrap();

    // deselect the chunk at the mouse position
    if let Some(chunk) = tree.get_chunk_from_position_mut(qv) {
        chunk.selected = false;
    }
}

fn main() {
    // set up the tree
    let mut tree = QuadTree::<Chunk, QuadVec>::with_capacity(32, 32);
    // start the glium event loop
    let event_loop = glutin::event_loop::EventLoop::new();
    let context = RenderContext::new(&event_loop);
    draw((0.5, 0.5), &mut tree, &context);

    // the mouse cursor position
    let mut mouse_pos = (0.5, 0.5);

    // last time redraw was done
    let mut last_redraw = std::time::Instant::now();

    // run the main loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = match event {
            glutin::event::Event::RedrawRequested(_) => {
                // and draw, if enough time elapses
                if last_redraw.elapsed().as_millis() > 16 {
                    draw(mouse_pos, &mut tree, &context);
                    last_redraw = std::time::Instant::now();
                }

                glutin::event_loop::ControlFlow::Wait
            }
            glutin::event::Event::WindowEvent { event, .. } => match event {
                // stop if the window is closed
                glutin::event::WindowEvent::CloseRequested => glutin::event_loop::ControlFlow::Exit,
                glutin::event::WindowEvent::CursorMoved { position, .. } => {
                    // get the mouse position
                    mouse_pos = (
                        position.x as f32 / context.display.get_framebuffer_dimensions().0 as f32,
                        position.y as f32 / context.display.get_framebuffer_dimensions().1 as f32,
                    );

                    // request a redraw
                    context.display.gl_window().window().request_redraw();

                    glutin::event_loop::ControlFlow::Wait
                }
                _ => glutin::event_loop::ControlFlow::Wait,
            },
            _ => glutin::event_loop::ControlFlow::Wait,
        }
    });
}
