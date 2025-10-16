extern crate proc_macro;

use proc_macro::{TokenStream};
use quote::{quote, ToTokens};

#[derive(Debug)]
struct FunctionArg {
    name: String,
    mutable: String,
    typename: String,
    reference: String
}

#[derive(Debug)]
struct FunctionParsed {
    _decl: String,
    name: String,
    args: Vec<FunctionArg>
}

// emits code to (move function args into closure, pass argments to the function)
fn emit_moves_and_pass_args(parsed: &FunctionParsed, omit_moves: &Vec<&str>) -> (String, String) {
    let mut moves = String::new();
    let mut pass = String::new();
    for (i, arg) in parsed.args.iter().enumerate() {
        let mut mutate = "";
        if arg.reference.find("mut").is_some() {
            mutate = "mut";
        }

        pass += &format!("{} {}", &arg.reference, &arg.name);
        if i < parsed.args.len()-1 {
            pass += ",\n";
        }

        let mut omit_move = false;
        for omit in omit_moves {
            if arg.typename.find(omit).is_some() {
                omit_move = true;
                break;
            }
        }

        if !omit_move {
            moves += &format!("{} {}: {}", mutate, &arg.name, &arg.typename);
            if i < parsed.args.len()-1 {
                moves += ",\n";
            }
        }
    }
    (moves, pass)
}

/// gets function info into strings, so it can be pasted and generated into wrapper functions 
fn parse_fn(item: &TokenStream) -> FunctionParsed {
    let function_item : syn::ItemFn = syn::parse(item.clone()).unwrap();

    let mut args = Vec::new();
    for input in &function_item.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            // get arg info into strings
            let mut arg = FunctionArg {
                name: String::new(),
                mutable: String::new(),
                typename: String::new(),
                reference: String::new()
            };

            // name and mutability
            match &*pat_type.pat {
                syn::Pat::Ident(ident) => {
                    arg.name = ident.ident.to_string();
                    if ident.mutability.is_some() {
                        arg.mutable = "mut".to_string();
                    }
                },
                _ => {}
            };

            // typename and reference
            match &*pat_type.ty {
                syn::Type::Reference(reference) => {
                    arg.typename = reference.elem.to_token_stream().to_string();
                    if reference.mutability.is_some() {
                        arg.reference = "&mut".to_string();
                    }
                    else {
                        arg.reference = "&".to_string();
                    }
                }
                _ => {
                    arg.typename = pat_type.ty.to_token_stream().to_string();
                }
            };
            args.push(arg);
                      
        }
    }

    FunctionParsed {
        _decl: "".to_string(),
        name: function_item.sig.ident.to_string(),
        args: args
    }
}

fn emit_update_order(attr: TokenStream, default_set: &str) -> String {
    if attr.to_string().contains("in_set") {
        attr.to_string()
    }
    else {
        if attr.is_empty() {
            format!("in_set ( {} )", default_set)
        }
        else {
            format!("in_set ( {} ) . {}", default_set, attr.to_string())
        }
    }
}

#[proc_macro_attribute]
pub fn export_update_fn(attr: TokenStream, item: TokenStream) -> TokenStream {    
    let parsed = parse_fn(&item);

    // emit code to move function args into closure and pass them to function
    let (moves, pass) = emit_moves_and_pass_args(&parsed, &Vec::new());

    let order = emit_update_order(attr, "SystemSets :: Update");

    // emit the closure code itself
    let export_fn = format!("#[no_mangle] fn export_{}() -> SystemConfigs {{
        (move | {} | {{
            {} ({}).unwrap();
        }}).into_configs().{}
    }}", parsed.name, moves, parsed.name, pass, order);

    // output the original item plus the generated export function
    let concat = format!(
        "{}\n{}", 
        item.to_string(),
        export_fn.to_string(),
    );
    
    concat.parse().unwrap()
}


#[proc_macro_attribute]
pub fn export_render_fn(attr: TokenStream, item: TokenStream) -> TokenStream {    
    let parsed = parse_fn(&item);

    // emit code to move function args into closure and pass them to function
    let (moves, pass) = emit_moves_and_pass_args(&parsed, &vec!["pmfx :: View"]);
    let order = emit_update_order(attr, "SystemSets :: Render");

    let render_closure = quote! {
        #[no_mangle] 
        fn export_fn_name(view_name: String) -> SystemConfigs {
            (move | fn_move | {
                let view = pmfx.get_view(&view_name);
                let err = match view {
                    Ok(v) => { 
                        let mut view = v.lock().unwrap();
                        
                        let col = view.colour_hash;
                        view.cmd_buf.begin_event(col, &view_name);

                        view.cmd_buf.begin_render_pass(&view.pass);
                        view.cmd_buf.set_viewport(&view.viewport);
                        view.cmd_buf.set_scissor_rect(&view.scissor_rect);

                        let result = fn_name(fn_args);

                        view.cmd_buf.end_render_pass();
                        view.cmd_buf.end_event();
                        result
                    }
                    Err(v) => {
                        Err(hotline_rs::Error {
                            msg: v.msg
                        })
                    }
                };

                // record errors
                if let Err(err) = err {
                    pmfx.log_error(&view_name, &err.msg);
                }
            }).into_configs().fn_attr
        }
    }.to_string();

    let export_fn = render_closure
        .replace("fn_move", &moves)
        .replace("fn_name", &parsed.name)
        .replace("fn_args", &pass)
        .replace("fn_attr", &order)
        .to_string();

    // output the original item plus the generated export function
    let concat = format!(
        "{}\n{}", 
        item.to_string(),
        export_fn.to_string(),
    );
    
    concat.parse().unwrap()
}

#[proc_macro_attribute]
pub fn export_compute_fn(attr: TokenStream, item: TokenStream) -> TokenStream {    
    let parsed = parse_fn(&item);

    // emit code to move function args into closure and pass them to function
    let (moves, pass) = emit_moves_and_pass_args(&parsed, &vec!["pmfx :: ComputePass"]);
    let order = emit_update_order(attr, "SystemSets :: Render");

    let render_closure = quote! {
        #[no_mangle] 
        fn export_fn_name(pass_name: String) -> SystemConfigs {
            (move | fn_move | {
                
                let pass = pmfx.get_compute_pass(&pass_name);
                let err = match pass {
                    Ok(p) => {
                        let mut pass = p.lock().unwrap();
                        pass.cmd_buf.begin_event(0xffffff, &pass_name);

                        let result = fn_name(fn_args);

                        pass.cmd_buf.end_event();

                        Ok(())
                    }
                    Err(p) => {
                        Err(hotline_rs::Error {
                            msg: p.msg
                        })
                    }
                };

                // record errors
                if let Err(err) = err {
                    pmfx.log_error(&pass_name, &err.msg);
                }
            }).into_configs().fn_attr
        }
    }.to_string();

    let export_fn = render_closure
        .replace("fn_move", &moves)
        .replace("fn_name", &parsed.name)
        .replace("fn_args", &pass)
        .replace("fn_attr", &order)
        .to_string();

    // output the original item plus the generated export function
    let concat = format!(
        "{}\n{}", 
        item.to_string(),
        export_fn.to_string(),
    );
    
    concat.parse().unwrap()
}