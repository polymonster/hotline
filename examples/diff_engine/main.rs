// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use maths_rs::vec::*;

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain};

use similar::{Algorithm, TextDiff, ChangeTag};

use std::{fs::File, io::Read, sync::RwLock, sync::Arc};

#[derive(Clone)]
struct LineDiff {
    pos: usize,
    len: usize,
    op: String
}

struct LineDiffs {
    lhs_diff: RwLock<Vec<LineDiff>>,
    rhs_diff: RwLock<Vec<LineDiff>>,
    lhs: String,
    rhs: String
}

fn format_string_lines(input: &str) -> Vec<String> {
    let sanitized = input.replace("\r\n", "\n").replace("\t", "    ");
    sanitized.split("\n").map(|x| x.to_string()).collect()
}

fn main() -> Result<(), hotline_rs::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("diff"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // device
    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });

    // window
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("diff!"),
        rect: os::Rect {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };
    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;
    let mut cmdbuffer = dev.create_cmd_buf(2);

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap().join("../..");

    let font_path = asset_path
        .join("data/fonts/consola.ttf")
        .to_str()
        .unwrap()
        .to_string();

    let mut imgui_info = imgui::ImGuiInfo {
        device: &mut dev,
        swap_chain: &mut swap_chain,
        main_window: &win,
        fonts: vec![imgui::FontInfo {
            filepath: font_path,
            glyph_ranges: None
        }],
    };
    let mut imgui = imgui::ImGui::create(&mut imgui_info).unwrap();

    let mut status_bar_height = 25.0;

    let mut lhs_file = File::open("C:\\Users\\gbdixonalex\\dev\\hotline\\examples\\diff_engine\\pipeline-verbosity1-lhs.txt")?;
    let mut rhs_file = File::open("C:\\Users\\gbdixonalex\\dev\\hotline\\examples\\diff_engine\\pipeline-verbosity1-rhs.txt")?;

    let mut lhs_string = String::new(); 
    lhs_file.read_to_string(&mut lhs_string)?;

    let mut rhs_string = String::new(); 
    rhs_file.read_to_string(&mut rhs_string)?;

    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(&lhs_string, &rhs_string);

    // separate the diffs
    let mut lhs_diff = Vec::new();
    let mut rhs_diff = Vec::new();
    for op in diff.ops() {
        for change in diff.iter_changes(op) {
            match change.tag() {
                ChangeTag::Delete => lhs_diff.push(format!("{}{}", "-", change)),
                ChangeTag::Insert => rhs_diff.push(format!("{}{}", "+", change)),
                ChangeTag::Equal => {
                    lhs_diff.push(format!("{}{}", " ", change));
                    rhs_diff.push(format!("{}{}", " ", change));
                }
            };
        }
    }

    // pad the diffs
    let mut lhs_pos = 0;
    let mut rhs_pos = 0;
    let iter_len = lhs_diff.len() + rhs_diff.len();

    let mut lhs_pad = false;
    let mut rhs_pad = false;

    for _ in 0..iter_len {
        if lhs_pos < lhs_diff.len() && rhs_pos < rhs_diff.len() {

            if lhs_diff[lhs_pos] == rhs_diff[rhs_pos] {
                if lhs_pad {
                    let pad_count = rhs_pos - lhs_pos;
                    for _ in 0..pad_count {
                        lhs_diff.insert(lhs_pos, String::from("\n"));
                        lhs_pos +=  1;
                    }
                    lhs_pad = false;
                }

                if rhs_pad {
                    let pad_count = lhs_pos - rhs_pos;
                    for _ in 0..pad_count {
                        rhs_diff.insert(rhs_pos, String::from("\n"));
                        rhs_pos +=  1;
                    }
                    rhs_pad = false;

                }
            }

            let lhs_sign = lhs_diff[lhs_pos].chars().nth(0).unwrap().to_ascii_lowercase();
            let rhs_sign =  rhs_diff[rhs_pos].chars().nth(0).unwrap().to_ascii_lowercase();

            if lhs_sign == ' ' && rhs_sign != ' ' {
                lhs_pad = true;
            }
            if lhs_sign != ' ' && rhs_sign == ' ' {
                rhs_pad = true;
            }
        }

        if !lhs_pad {
            lhs_pos += 1
        }

        if !rhs_pad {
            rhs_pos += 1;
        }
    }

    assert_eq!(lhs_diff.len(), rhs_diff.len());

    let lines_diffs: Arc<Vec<LineDiffs>> = Arc::new(
        (0..lhs_diff.len()).enumerate().map(|(index, _)| LineDiffs {
            lhs_diff: RwLock::new(Vec::new()),
            rhs_diff: RwLock::new(Vec::new()),
            lhs: lhs_diff[index].clone(),
            rhs: rhs_diff[index].clone()
        }).collect()
    );

    let line_count = lhs_diff.len();

    // async work
    let mut handles = vec![];
    let lines_clone = Arc::clone(&lines_diffs);
    handles.push(std::thread::spawn(move || {
        for i in 0..line_count {
            let diff = TextDiff::from_words(&lines_diffs[i].lhs, &lines_diffs[i].rhs);
            let mut lhs_pos = 0;
            let mut rhs_pos = 0;
            if i == 32 {
                println!("{}", lines_diffs[i].lhs);
                println!("{}", lines_diffs[i].rhs);
            }
            for op in diff.ops() {
                for change in diff.iter_changes(op) {
                    let change_str = change.as_str().unwrap();
                    match change.tag() {
                        ChangeTag::Delete => { 
                            if i == 32 {
                                println!("Delete {} -> {}", lhs_pos, change_str.len());
                            }
                            lines_diffs[i].lhs_diff.write().expect("").push(LineDiff {
                                op: "-".to_string(),
                                pos: lhs_pos,
                                len: change_str.len()
                            });
                            lhs_pos += change_str.len()
                        },
                        ChangeTag::Insert => {
                            if i == 32 {
                                println!("Insert {} -> {}", lhs_pos, change_str.len());
                            }
                            lines_diffs[i].rhs_diff.write().expect("").push(LineDiff {
                                op: "+".to_string(),
                                pos: rhs_pos,
                                len: change_str.len()
                            });
                            rhs_pos += change_str.len();
                        },
                        ChangeTag::Equal => {
                            if i == 32 {
                                println!("Equal {} -> {}", lhs_pos, change_str.len());
                            }
                            lhs_pos += change_str.len();
                            rhs_pos += change_str.len();
                        }
                    }
                }
            }
    }}));


    // 
    let mut scroll_y = 0.0;

    // ..
    let mut ci = 0;
    while app.run() {

        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // main pass
        cmdbuffer.begin_event(0xff0000ff, "Main Pass");

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(pass);

        // imgui
        imgui.new_frame(&mut app, &mut win, &mut dev);

        // dock
        imgui.add_main_dock(25.0);
        status_bar_height = imgui.add_status_bar(status_bar_height);

        let mut open = true;

        let diff_panes = [
            ("lhs", &lhs_diff),
            ("rhs", &rhs_diff)
        ];

        for pane in diff_panes {
            let (name, diff) = pane;
            imgui.begin(name, &mut open, imgui::WindowFlags::ALWAYS_HORIZONTAL_SCROLLBAR);
            if name == "rhs" {
                imgui.set_scroll_y(scroll_y)
            }
            else {
                scroll_y = imgui.get_scroll_y();
            }
            for (index, line) in diff.iter().enumerate() {

                imgui.text(&format!("{}", index));
                imgui.same_line();


                if name == "rhs" {
                    if index == 32 {
                        println!("{}", line);
                    }
                    let line_diffs = lines_clone[index].rhs_diff.read().expect("").to_vec();
                    let mut marks = Vec::new();
                    marks.resize_with(line.len(), || {
                        0
                    });

                    for line_diff in &line_diffs {
                        imgui.highlight_text(&line, line_diff.pos, line_diff.len);

                        for i in line_diff.pos..(line_diff.pos+line_diff.len) {
                            marks[i] = 1;
                        }
                    }

                    if index == 32 {
                        for m in &marks {
                            if *m == 0 {
                                print!(" ");
                            }   
                            else {
                                print!("_");
                            }
                        }
                        println!("");
                    }
                }
                else {
                    let line_diffs = lines_clone[index].lhs_diff.read().expect("").to_vec();
                    for line_diff in &line_diffs {
                        imgui.highlight_text(&line, line_diff.pos, line_diff.len);
                    }
                }

                imgui.same_line();
                if line.chars().nth(0).unwrap() == '-' {
                    imgui.colour_text(&line, vec4f(1.0, 0.0, 0.0, 1.0));
                }
                else if line.chars().nth(0).unwrap() == '+' {
                    imgui.colour_text(&line, vec4f(0.0, 1.0, 0.0, 1.0));
                }
                else {
                    imgui.text(&line);
                }
            }
            imgui.end();
        }
        
        imgui.render(&mut app, &mut win, &mut dev, &mut cmdbuffer, &Vec::new());

        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        cmdbuffer.end_event();

        cmdbuffer.close()?;

        dev.execute(&cmdbuffer);

        swap_chain.swap(&dev);
        ci = (ci + 1) % 4;
    }

    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    dev.cleanup_dropped_resources(&swap_chain);
    
    Ok(())
}
