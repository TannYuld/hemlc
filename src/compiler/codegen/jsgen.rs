use crate::compiler::{
    codegen::{
        types::{CodegenStrategy, Compiler},
        util,
    },
    obfuscation::ObfuscatedExpr,
};

pub trait JsGenerator {
    fn slot_element_replace(&self, name: &ObfuscatedExpr) -> String;

    fn component_initialization(
        &self,
        func_name: &str,
        target_marker: &ObfuscatedExpr,
        js_props: &str,
        component_content: &str,
        variable_zone: &str,
        binding_zone: &str,
    ) -> String;
    fn component_event_handling(&self, id: &str, js_event: &str, logic: &str) -> String;
    fn component_function_declaration(
        &self,
        func_name: &str,
        html_body: &str,
        js_vars: &str,
        js_var_bindings: &str,
        nested_functions: &str,
    ) -> String;
    fn component_attributes(&self, name: &str) -> String;

    fn fixed_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String;
    fn reactive_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String;

    fn dependecy_binding(&self, dependency: &str, expr: &ObfuscatedExpr) -> String;

    fn generate_variable_decleration(&self, var_name: &str, val: &Option<String>) -> String;
    fn generate_conditional_block(
        &self,
        expr: &ObfuscatedExpr,
        condition_bodies: &str,
        dependency_bindings: &str,
    ) -> String;
    fn generate_condition_body(&self, condition: &str, body: &str) -> String;
    fn generate_block_body(&self, vars: &str, html: &str, bindings: &str) -> String;

    fn assemble_js(&self) -> String;
    fn expr_binding(&self, expr: &ObfuscatedExpr, deps_array: &str, final_expr: &str) -> String;
}

impl JsGenerator for Compiler {
    fn slot_element_replace(&self, name: &ObfuscatedExpr) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => {
                format!(
                    "
    const m_{0} = FindMarker('{0}', frag);
    if (slot_frag) {{
        m_{0}.replaceWith(slot_frag);
    }}else {{
        m_{0}.remove();
    }}
    ",
                    name.0
                )
            }
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => {
                format!(
                    "const m_{0}=FindMarker('{0}',frag);if(slot_frag){{m_{0}.replaceWith(slot_frag);}}else{{m_{0}.remove();}}",
                    name.0
                )
            }
        }
    }

    fn component_initialization(
        &self,
        func_name: &str,
        target_marker: &ObfuscatedExpr,
        js_props: &str,
        slot_content: &str,
        variable_zone: &str,
        binding_zone: &str,
    ) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => {
                format!(
                    "
    const target_{0} = FindMarker('{0}', frag);
    const slot_frag_{0} = (() => {{ 
        {1}
        const frag = HtmlToFragment(`{2}`);
        {3}
        return frag;
    }})();
    {4}(target_{0}, {5}, slot_frag_{0});
    ",
                    target_marker.0, variable_zone, slot_content, binding_zone, func_name, js_props
                )
            }
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => {
                format!(
                    "const target_{0}=FindMarker('{0}',frag);const slot_frag_{0}=(()=>{{{1}const frag=HtmlToFragment(`{2}`);{3}return frag;}})();{4}(target_{0},{5},slot_frag_{0});",
                    target_marker.0, variable_zone, slot_content, binding_zone, func_name, js_props
                )
            }
        }
    }

    fn fixed_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    const marker__{0} = FindMarker(\"{0}\", frag);
    marker__{0}.after(document.createTextNode({1}.value{2}));
    ",
                var_name.0, base_var, remainder
            ),
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
                "const marker__{0}=FindMarker(\"{0}\",frag);marker__{0}.after(document.createTextNode({1}.value{2}));",
                var_name.0, base_var, remainder
            ),
        }
    }

    fn reactive_value(&self, var_name: &ObfuscatedExpr, base_var: &str, remainder: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    const marker__{0} = FindMarker(\"{0}\", frag);
    BindValue(marker__{0}, {1}, () => ({1}.value{2}));
    ",
                var_name.0, base_var, remainder
            ),
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
                "const marker__{0}=FindMarker(\"{0}\",frag);BindValue(marker__{0},{1},()=>({1}.value{2}));",
                var_name.0, base_var, remainder
            ),
        }
    }

    fn component_event_handling(&self, id: &str, js_event: &str, logic: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}', async (event) => {{
        {}
    }});
    ",
                id, js_event, logic
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
                "frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}',async (event)=>{{{}}});",
                id, js_event, logic
            ),
        }
    }

    fn component_function_declaration(
        &self,
        func_name: &str,
        html_body: &str,
        js_vars: &str,
        js_var_bindings: &str,
        nested_functions: &str,
    ) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    {}
    function {}(marker_target, props, slot_frag) {{
        {}
        const frag = HtmlToFragment(`{}`);
        {}
        marker_target.after(frag);
    }}
    ",
                nested_functions, func_name, js_vars, html_body, js_var_bindings
            ),
            CodegenStrategy::MinifyJsOnly | CodegenStrategy::MinifyAll => format!(
                "{}function {}(marker_target,props,slot_frag){{{}const frag=HtmlToFragment(`{}`);{}marker_target.after(frag);}}",
                nested_functions, func_name, js_vars, html_body, js_var_bindings
            ),
        }
    }

    fn component_attributes(&self, name: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "const {0} = (props.{0} !== undefined && props.{0}.addSubscriber) ? props.{0} : Observable(props.{0});",
                name
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(
                "const {0}=(props.{0}!==undefined&&props.{0}.addSubscriber)?props.{0}:Observable(props.{0});",
                name
            ),
        }
    }

    fn assemble_js(&self) -> String {
        let new_line = if self.options.codegen_strategy == CodegenStrategy::AsIs {
            "\n"
        } else {
            ""
        };
        let mut result = String::new();
        result += &self.buffer.js.component_function_zone;
        result += new_line;
        result += &self.buffer.js.var_zone;
        result += new_line;
        result += "\tconst frag=document;";
        result += new_line;
        result += &self.buffer.js.binding_zone;
        result
    }

    fn generate_variable_decleration(&self, name: &str, val: &Option<String>) -> String {
        let val_expr = if let Some(val) = val {
            util::parse_js_expr(val)
        } else {
            "undefined".to_string()
        };

        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!("\tconst {} = Observable({});", name, val_expr),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => {
                format!("const {}=Observable({});", name, val_expr)
            }
        }
    }

    fn generate_condition_body(&self, condition: &str, body: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    {{
        condition: () => {0},
        evaluation: () => {{
            {1}
        }}
    }},
    ",
                condition, body
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(""),
        }
    }

    fn generate_conditional_block(
        &self,
        expr: &ObfuscatedExpr,
        condition_bodies: &str,
        dependency_bindings: &str,
    ) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    const markers_{0} = FindLimitMarkers('{0}', frag);
    const update_{0} = If(markers_{0}, [
        {1}
    ]);
    update_{0}();
    {2}
    ",
                expr.0, condition_bodies, dependency_bindings
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(""),
        }
    }

    fn dependecy_binding(&self, dependency: &str, expr: &ObfuscatedExpr) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!("{}.addSubscriber(update_{});", dependency, expr.0),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(""),
        }
    }

    fn expr_binding(&self, expr: &ObfuscatedExpr, deps_array: &str, final_expr: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "BindExpression(FindMarker('{}', frag), {}, () => ({}));",
                expr.0, deps_array, final_expr
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(""),
        }
    }

    fn generate_block_body(&self, vars: &str, html: &str, bindings: &str) -> String {
        match self.options.codegen_strategy {
            CodegenStrategy::AsIs => format!(
                "
    {}
    const frag = HtmlToFragment(`{}`);
    {}
    return frag;
    ",
                vars, html, bindings
            ),
            CodegenStrategy::MinifyAll | CodegenStrategy::MinifyJsOnly => format!(""),
        }
    }
}
