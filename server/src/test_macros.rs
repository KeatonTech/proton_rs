/// Builds a static hashmap
macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
        };
);

/// Converts a rust variable type to the NodeValueType that could hold it
macro_rules! node_value_of {
    ($val:ident: TriggerSignal) => {NodeValue::Trigger($val)};
    ($val:literal: TriggerSignal) => {NodeValue::Trigger($val)};
    ($val:ident: bool) => {NodeValue::Toggle($val)};
    ($val:literal: bool) => {NodeValue::Toggle($val)};
    ($val:ident: i64) => {NodeValue::Count($val)};
    ($val:literal: i64) => {NodeValue::Count($val)};
    ($val:ident: u32) => {NodeValue::ConstrainedMagnitude($val)};
    ($val:literal: u32) => {NodeValue::ConstrainedMagnitude($val)};
    ($val:ident: f64) => {NodeValue::UnconstrainedMagnitude($val)};
    ($val:literal: f64) => {NodeValue::UnconstrainedMagnitude($val)};
    ($val:ident: NodeColor) => {NodeValue::Color($val)};
    ($val:literal: NodeColor) => {NodeValue::Color($val)};
    ($val:ident: Box<String>) => {NodeValue::Text($val)};
    ($val:literal: Box<String>) => {NodeValue::Text($val)};
}

/// Converts a rust variable type to the NodeValueType that could hold it
macro_rules! node_value_type_of {
    (TriggerSignal) => {NodeValueType::Trigger};
    (bool) => {NodeValueType::Toggle};
    (i64) => {NodeValueType::Count};
    (u32) => {NodeValueType::ConstrainedMagnitude};
    (f64) => {NodeValueType::UnconstrainedMagnitude};
    (NodeColor) => {NodeValueType::Color};
    (Box<String>) => {NodeValueType::Text};
}

/// Converts an arg to a NodeValueInput
macro_rules! node_input_def_from_arg {
    ($name:ident: $type:ident) => {
        NodeInputDef {
            desc: NodeDefBasicDescription {
                name: stringify!($name).to_string(),
                description: concat!("Automatic description of input ", stringify!($name)).to_string(),
            },
            allowed_types: vec![node_value_type_of!($type)],
            required: true,
        }
    };
}

/// Makes a list of NodeValueInputs based on function args.
macro_rules! node_input_def_from_args {
    ($($name:ident: $type:ident),*) => {vec![
        $(node_input_def_from_arg!($name: $type)),*
    ]};
}

/// Converts an arg to a NodeValueInput
macro_rules! node_output_def_from_type {
    ($type:ident) => {
        NodeOutputDef {
            desc: NodeDefBasicDescription {
                name: "Generic output".to_string(),
                description: "Generic output description".to_string()
            },
            output_type: node_value_type_of!($type),
        }
    };
}

/// Makes a list of NodeValueInputs based on function args.
macro_rules! node_output_def_from_tuple {
    ($($type:ident),+) => {vec![
        $(node_output_def_from_type!($type)),+
    ]};
}

/// Wraps a given function body with unwrapping code for NodeValue inputs
macro_rules! wrap_node_function {
    (@body {$body:block} $ivar:ident $name_1:ident: $type_1:ident $idx:expr) => {
        if let node_value_of!($name_1: $type_1) = $ivar[$idx] {
            $body
        } else {
            panic!(concat!("Invalid type for NodeValue input ", stringify!($name_1)));
        }
    };

    (@body {$body:block} $ivar:ident $name_1:ident: $type_1:ident, $($name:ident: $type:ident),+ $idx:expr) => {
        if let node_value_of!($name_1: $type_1) = $ivar[$idx] {
            wrap_node_function!(@body {$body} $ivar $($name: $type),+ $idx + 1usize)
        } else {
            panic!(concat!("Invalid type for NodeValue input ", stringify!($name_1)));
        }
    };

    (fn $fname:ident($($name:ident: $type:ident),+) -> $o:ty $body:block) => {
        fn $fname(inputs: Vec<&NodeValue>) -> $o {
            wrap_node_function!(@body {$body} inputs $($name: $type),+ 0)
        }
    };

    (|$($name:ident: $type:ident),+| $body:block) => {
        |inputs: Vec<&NodeValue>| {
            wrap_node_function!(@body {$body} inputs $($name: $type),+ 0)
        }
    };

    (fn $fname:ident( ) -> $o:ty $body:block) => {
        fn $fname(_inputs: Vec<&NodeValue>) -> $o {
            $body
        }
    };

    (| | $body:block) => {
        |_inputs: Vec<&NodeValue>| {
            $body
        }
    };
}

/// Builds a NodeDef with a function runner from a lambda function.
macro_rules! node_def_from_fn {
    (|$($name:ident: $type:ident),*| -> ($($o:ident),+) $body:block) => {
        NodeDef {
            desc: NodeDefBasicDescription {
                name: "Test Node".to_string(),
                description: "Test Description".to_string(),
            },
            inputs: node_input_def_from_args!($($name: $type),*),
            outputs: node_output_def_from_tuple!($($o),+),
            runner: NodeDefRunner::Function(wrap_node_function!(|$($name: $type),*| $body))
        }
    };

    (|| -> ($($o:ident),+) $body:block) => {
        node_def_from_fn!(| | -> ($($o),+) $body);
    };

    (fn $fname:ident($($name:ident: $type:ident),*) -> ($($o:ident),+) $body:block) => {
        NodeDef {
            desc: NodeDefBasicDescription {
                name: stringify!($fname).to_string(),
                description: concat!("Automatic description of node ", stringify!($fname)).to_string(),
            },
            inputs: node_input_def_from_args!($($name: $type),+),
            outputs: node_output_def_from_tuple!($($o),+),
            runner: NodeDefRunner::Function(wrap_node_function!(|$($name: $type),+| $body))
        }
    };
}

/// Instantiates a node with an Id, a def, and inputs.
macro_rules! make_node {
    (@input Wire{$nodeid:literal, $output:literal}) => {
        NodeInput::Wire(NodeOutputRef {
            from_node_id: $nodeid,
            node_output_index: $output
        })
    };
    (@input $type:ident{$val:literal}) => {
        NodeInput::Const(node_value_of!($val: $type))
    };
    ($id:literal: $def:ident[$($type:ident{$($arg:literal),+}),*]) => {
        Node {
            id: $id,
            def_name: stringify!($def).to_string(),
            inputs: vec![
                $(make_node!(@input $type{$($arg),+})),*
            ]
        }
    };
}

/// Instantiates one or more nodes with Ids, defs, and inputs.
macro_rules! make_nodes {
    ($($id:literal: $def:ident[$($type:ident{$($arg:literal),+}),*]),+) => {
        vec![
            $(make_node!($id: $def[$($type{$($arg),+}),*])),+
        ]
    };
}