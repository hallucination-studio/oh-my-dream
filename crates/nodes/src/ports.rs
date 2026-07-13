use engine::{InputPort, OutputPort, PortCardinality, PortType};

pub(crate) fn required_input(name: &str, port_type: PortType) -> InputPort {
    InputPort {
        name: name.to_owned(),
        port_type,
        cardinality: PortCardinality::One,
        required: true,
        default: None,
    }
}

pub(crate) fn required_many_input(
    name: &str,
    port_type: PortType,
    minimum: usize,
    maximum: Option<usize>,
) -> InputPort {
    InputPort {
        name: name.to_owned(),
        port_type,
        cardinality: PortCardinality::Many { minimum, maximum },
        required: true,
        default: None,
    }
}

pub(crate) fn output(name: &str, port_type: PortType) -> OutputPort {
    OutputPort { name: name.to_owned(), port_type }
}
