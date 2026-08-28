use crate::compiler::{
    codegen::{
        HEML_ID_ATTRIBUTE_KEY,
        types::{
            Compiler, JSGenerator,
            minify::{self, None},
        },
        util,
    },
    obfuscation::ObfuscatedExpr,
};
use indoc::indoc;
use std::fmt::Write;

impl JSGenerator for minify::None {
    fn write_js_element_event(out: &mut String, events: super::types::HtmlEventList, id: &str) {
        for (event_name, event_body, is_async) in events.iter() {
            let _ = write!(
                out,
                r#"    frag.querySelector('[{}="{}"]').addEventListener('{}',{} (event) => {{
        {}
    }});
"#,
                HEML_ID_ATTRIBUTE_KEY, id, event_name, if *is_async {" async"} else {""}, event_body
            );
        }
    }

    fn write_global_scope(out: &mut String) {
        let _ = write!(out, "\tconst frag = document;");
    }
}

impl JSGenerator for minify::Js {
    fn write_js_element_event(out: &mut String, events: super::types::HtmlEventList, id: &str) {
        for (event_name, event_body, is_async) in events.iter() {
            let _ = write!(
                out,
                r#"frag.querySelector('[{}="{}"]').addEventListener('{}',{}(event)=>{{{}}});"#,
                HEML_ID_ATTRIBUTE_KEY, id, event_name, if *is_async {"async"} else {""}, event_body
            );
        }
    }

    fn write_global_scope(out: &mut String) {
        let _ = write!(out, "const frag=document;");
    }
}

impl JSGenerator for minify::All {
    fn write_js_element_event(out: &mut String, events: super::types::HtmlEventList, id: &str) {
        minify::Js::write_js_element_event(out, events, id);
    }

    fn write_global_scope(out: &mut String) {
        minify::Js::write_global_scope(out);
    }
}

// pub trait JsGenerator {
//     fn slot_element_replace(&self, name: &ObfuscatedExpr, args: &str, slot_name: &str) -> String;

//     fn component_initialization(
//         &self,
//         func_name: &str,
//         target_marker: &ObfuscatedExpr,
//         js_props: &str,
//         slot_factories: &str,
//         variable_zone: &str,
//     ) -> String;
//     fn component_event_handling(&self, id: &str, js_event: &str, logic: &str) -> String;
//     fn component_function_declaration(
//         &self,
//         func_name: &str,
//         html_body: &str,
//         js_vars: &str,
//         js_var_bindings: &str,
//         nested_functions: &str,
//     ) -> String;
//     fn component_attributes(&self, name: &str) -> String;

//     fn dependecy_binding(&self, dependency: &str, expr: &ObfuscatedExpr) -> String;

//     fn generate_variable_decleration(&self, var_name: &str, val: &Option<String>) -> String;
//     fn generate_conditional_block(
//         &self,
//         expr: &ObfuscatedExpr,
//         condition_bodies: &str,
//         dependency_bindings: &str,
//     ) -> String;
//     fn generate_condition_body(&self, condition: &str, body: &str) -> String;
//     fn generate_block_body(&self, vars: &str, html: &str, bindings: &str) -> String;

//     fn generate_slot_factory(&self, html: &str, bindings: &str) -> String;

//     fn assemble_js(&self) -> String;
//     fn expr_binding(&self, expr: &ObfuscatedExpr, deps_array: &str, final_expr: &str) -> String;
// }

// impl JsGenerator for Compiler {
//     fn slot_element_replace(&self, name: &ObfuscatedExpr, args: &str, slot_name: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => {
//                 format!(
//                     "
//     const m_{0} = FindMarker('{0}', frag);
//     if (slots && typeof slots['{2}'] === 'function') {{
//         m_{0}.replaceWith(slots['{2}']({1}));
//     }} else {{
//         m_{0}.remove();
//     }}
//     ",
//                     name.0, args, slot_name
//                 )
//             }
//             CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => {
//                 format!(
//                     "const m_{0}=FindMarker('{0}',frag);if(slots&&typeof slots['{2}']==='function'){{m_{0}.replaceWith(slots['{2}']({1}));}}else{{m_{0}.remove();}}",
//                     name.0, args, slot_name
//                 )
//             }
//         }
//     }

//     fn generate_slot_factory(&self, html: &str, bindings: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "(slotProps = {{}}) => {{\n        const frag = HtmlToFragment(`{}`);\n        {}\n        return frag;\n    }}",
//                 html, bindings
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "(slotProps={{}})=>{{const frag=HtmlToFragment(`{}`);{}return frag;}}",
//                 html, bindings
//             ),
//         }
//     }

//     fn component_initialization(
//         &self,
//         func_name: &str,
//         target_marker: &ObfuscatedExpr,
//         js_props: &str,
//         slot_factories: &str,
//         variable_zone: &str,
//     ) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => {
//                 format!(
//                     "
//     const target_{0} = FindMarker('{0}', frag);
//     {1}
//     const slots_{0} = {{
//         {2}
//     }};
//     {3}(target_{0}, {4}, slots_{0});
//     ",
//                     target_marker.0, variable_zone, slot_factories, func_name, js_props
//                 )
//             }
//             CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => {
//                 format!(
//                     "const target_{0}=FindMarker('{0}',frag);{1}const slots_{0}={{{2}}};{3}(target_{0},{4},slots_{0});",
//                     target_marker.0, variable_zone, slot_factories, func_name, js_props
//                 )
//             }
//         }
//     }

//     fn component_event_handling(&self, id: &str, js_event: &str, logic: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "
//     frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}', async (event) => {{
//         {}
//     }});
//     ",
//                 id, js_event, logic
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}',async (event)=>{{{}}});",
//                 id, js_event, logic
//             ),
//         }
//     }

//     fn component_function_declaration(
//         &self,
//         func_name: &str,
//         html_body: &str,
//         js_vars: &str,
//         js_var_bindings: &str,
//         nested_functions: &str,
//     ) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "
//     {}
//     function {}(marker_target, props, slots) {{
//         const children = slots;
//         {}
//         const frag = HtmlToFragment(`{}`);
//         {}
//         marker_target.after(frag);
//     }}
//     ",
//                 nested_functions, func_name, js_vars, html_body, js_var_bindings
//             ),
//             CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
//                 "{}function {}(marker_target,props,slots){{const children=slots;{}const frag=HtmlToFragment(`{}`);{}marker_target.after(frag);}}",
//                 nested_functions, func_name, js_vars, html_body, js_var_bindings
//             ),
//         }
//     }

//     fn component_attributes(&self, name: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "const {0} = (props.{0} !== undefined && props.{0}.addSubscriber) ? props.{0} : Observable(props.{0});",
//                 name
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "const {0}=(props.{0}!==undefined&&props.{0}.addSubscriber)?props.{0}:Observable(props.{0});",
//                 name
//             ),
//         }
//     }

//     fn assemble_js(&self) -> String {
//         let new_line = if self.options.codegen_strategy == CodegenStrategy::AsIs {
//             "\n"
//         } else {
//             ""
//         };
//         let mut result = String::new();
//         result += &self.buffer.js.component_function_zone;
//         result += new_line;
//         result += &self.buffer.js.var_zone;
//         result += new_line;
//         result += "\tconst frag=document;";
//         result += new_line;
//         result += &self.buffer.js.binding_zone;
//         result
//     }

//     fn generate_variable_decleration(&self, name: &str, val: &Option<String>) -> String {
//         let val_expr = if let Some(val) = val {
//             util::parse_js_expr(val)
//         } else {
//             "undefined".to_string()
//         };

//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!("\tconst {} = Observable({});", name, val_expr),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => {
//                 format!("const {}=Observable({});", name, val_expr)
//             }
//         }
//     }

//     fn generate_condition_body(&self, condition: &str, body: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "
//     {{
//         condition: () => {0},
//         evaluation: () => {{
//             {1}
//         }}
//     }},
//     ",
//                 condition, body
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "{{condition:()=>{0},evaluation:()=>{{{1}}}}},",
//                 condition, body
//             ),
//         }
//     }

//     fn generate_conditional_block(
//         &self,
//         expr: &ObfuscatedExpr,
//         condition_bodies: &str,
//         dependency_bindings: &str,
//     ) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "
//     const markers_{0} = FindLimitMarkers('{0}', frag);
//     const update_{0} = If(markers_{0}, [
//         {1}
//     ]);
//     update_{0}();
//     {2}
//     ",
//                 expr.0, condition_bodies, dependency_bindings
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "const markers_{0}=FindLimitMarkers('{0}',frag);const update_{0}=If(markers_{0},[{1}]);update_{0}();{2}",
//                 expr.0, condition_bodies, dependency_bindings
//             ),
//         }
//     }

//     fn dependecy_binding(&self, dependency: &str, expr: &ObfuscatedExpr) -> String {
//         format!("{}.addSubscriber(update_{});", dependency, expr.0)
//     }

//     fn expr_binding(&self, expr: &ObfuscatedExpr, deps_array: &str, final_expr: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "BindExpression(FindMarker('{}', frag), {}, () => ({}));",
//                 expr.0, deps_array, final_expr
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "BindExpression(FindMarker('{}',frag),{},()=>({}));",
//                 expr.0, deps_array, final_expr
//             ),
//         }
//     }

//     fn generate_block_body(&self, vars: &str, html: &str, bindings: &str) -> String {
//         match self.options.codegen_strategy {
//             CodegenStrategy::AsIs => format!(
//                 "
//     {}
//     const frag = HtmlToFragment(`{}`);
//     {}
//     return frag;
//     ",
//                 vars, html, bindings
//             ),
//             CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
//                 "{}const frag = HtmlToFragment(`{}`);{}return frag;",
//                 vars, html, bindings
//             ),
//         }
//     }
// }
