pub struct Mcfunction {
    pub name: String,
    pub body: Vec<String>,
}

pub struct DataPack {
    pub functions: Vec<Mcfunction>,
}

pub struct DataPackBuilder {
    committed_functions: Vec<Mcfunction>,
    stack: Vec<Mcfunction>,
}

impl DataPackBuilder {
    pub fn new() -> Self {
        Self {
            committed_functions: Vec::new(),
            stack: Vec::new(),
        }
    }

    pub fn push_function(&mut self, name: String) -> &mut Self {
        self.stack.push(Mcfunction {
            name,
            body: Vec::new(),
        });
        self
    }

    pub fn pop_function(&mut self) -> &mut Self {
        match self.stack.pop() {
            Some(function) => {
                self.committed_functions.push(function);
            }
            None => {}
        }
        self
    }

    pub fn push_command(&mut self, command: String) -> &mut Self {
        match self.stack.last_mut() {
            Some(function) => {
                function.body.push(command);
            }
            None => {}
        }
        self
    }

    pub fn extend_commands(&mut self, commands: Vec<String>) -> &mut Self {
        match self.stack.last_mut() {
            Some(function) => {
                function.body.extend(commands);
            }
            None => {}
        }
        self
    }

    pub fn complete(self) -> DataPack {
        DataPack {
            functions: self.committed_functions,
        }
    }
}
