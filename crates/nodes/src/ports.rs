use engine::{InputPort, OutputPort, PortType};

pub(crate) fn required_input(name: &str, port_type: PortType) -> InputPort {
    InputPort { name: name.to_owned(), port_type, required: true, default: None }
}

pub(crate) fn output(name: &str, port_type: PortType) -> OutputPort {
    OutputPort { name: name.to_owned(), port_type }
}
