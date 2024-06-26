use error_chain::error_chain;

error_chain! {
    links {
    }

    foreign_links {
        File(std::io::Error);
        Serialize(ark_serialize::SerializationError);
    }

    errors {
        InconsistentLength {
            description("In consistent length between expected params and real params")
            display("In consistent length between expected params and real params")
        }
    }
}
