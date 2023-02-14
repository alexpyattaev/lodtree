use std::f32::consts::PI;
use std::thread::sleep;
use std::time::Duration;
use glium::index::PrimitiveType;
use glium::{glutin, implement_vertex, program, uniform, Surface, Program, Display, VertexBuffer, IndexBuffer};
use glium::glutin::event_loop::EventLoop;

use lodtree::coords::QuadVec;
use lodtree::*;

// the chunk struct for the tree
struct Chunk {
    visible: bool,
    cache_state: i32,
    // 0 is new, 1 is merged, 2 is cached, 3 is both
    selected: bool,
    in_bounds: bool,
}

fn make_shaders(display: &Display) -> Program
{
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
    pub fn new( event_loop: &EventLoop<()>) -> Self
    {

        let wb = glutin::window::WindowBuilder::new().with_title("Quadtree demo");
        let cb = glutin::ContextBuilder::new().with_vsync(true);
        let display = glium::Display::new(wb, cb, &event_loop).unwrap();
        // make a vertex buffer
        // we'll reuse it as we only need to draw one quad multiple times anyway
        let vertex_buffer = {
            glium::VertexBuffer::new(
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
        let index_buffer = glium::IndexBuffer::new(
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


fn draw(mouse_pos: (f32, f32),
        tree: &mut Tree<Chunk, QuadVec>,
        ctx: &RenderContext) {



    // update the tree
    // adding chunks to their respective position, and also set them visible when adding
    fn chunk_creator(position:QuadVec)->Chunk
    {
        let R = 6;

        let visible =  match position.depth {
                  4=> ((position.x as i32 - R).pow(2) + (position.y as i32 - R).pow(2) < R),
            _=>false,
        };

        // dbg!(position);
        //  dbg!(visible);
         Chunk {
            visible:true,
            cache_state: visible as i32,
            selected: false,
            in_bounds: false,
        }
    }
    let qv = QuadVec::new(6,6,4);
    if tree.prepare_update(
        &[qv],
        6,
        chunk_creator,
    ) {
        // position should already have been set, so we can just change the visibility
         for chunk in tree.iter_chunks_to_activate_mut() {
             chunk.visible = false;
        //     chunk.cache_state |= 1;
        }

        for chunk in tree.iter_chunks_to_deactivate_mut() {
            chunk.visible = false;
        }

        // and make chunks that are cached visible
        for chunk in tree.iter_chunks_to_remove_mut() {
            chunk.cache_state = 2;
        }

        // do the update
        tree.do_update();

        // and clean
        tree.complete_update();
    }
    dbg!("Searching!");
    tree.search_around(qv, 7, | c|{
        c.chunk.cache_state = 4
    });
    let min = QuadVec::new(0, 0, 4);
    let max = QuadVec::new(2, 2, 4);
    let mut count = 0;

    for i in tree.iter_all_chunks_in_bounds_and_tree_mut(min, max, 4){

        //sleep(Duration::from_secs_f32(0.1));

        if !i.0.contains_child_node(QuadVec::new(i.0.x<<1, i.0.y<<1, 4))&&i.1.visible {

            println!(" YES CHUNK x: {:?} y: {:?}, on depth: {:?}", i.0.x, i.0.y, i.0.depth);
            count += 1;
        }

        else{println!(" NO CHUNK  x: {:?} y: {:?}, on depth: {:?}", i.0.x, i.0.y, i.0.depth);
        count += 1;}
        println!("{:?}", count);
    }





    // go over all chunks in the tree and set them to not be selected
    for chunk in tree.iter_chunks_mut() {
        chunk.selected = false;
    }

    // and select the chunk at the mouse position
    // if let Some(chunk) = tree.get_chunk_from_position_mut(qv) {
    //     chunk.selected = true;
    //     chunk.visible = true;
    // }

    // and select a number of chunks in a region when the mouse buttons are selected

    // and, Redraw!
    let mut target = ctx.display.draw();
    target.clear_color(0.6, 0.6, 0.6, 1.0);

    // go over all chunks, iterator version
    for (chunk, position) in tree.iter_chunks_and_positions() {
        if chunk.visible {
            // draw it if it's visible
            // here we get the chunk position and size
            let uniforms = uniform! {
                    offset: [position.get_float_coords().0 as f32, position.get_float_coords().1 as f32],
                    scale: position.get_size() as f32,
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
}


fn main() {
    // set up the tree
    let mut tree = Tree::<Chunk, QuadVec>::new(0);
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
