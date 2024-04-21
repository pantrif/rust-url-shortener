	static ALPHABET: &'static str = "23456789bcdfghjkmnpqrstvwxyzBCDFGHJKLMNPQRSTVWXYZ-_";
	static BASE: usize = 51;

	pub fn encode(mut id: usize) -> String {
		let mut string: String = format!("");
		while id > 0 {
			string.push_str(&ALPHABET[(id % BASE)..(id % BASE + 1)]);
			id = id / BASE;
		}
		string.chars().rev().collect()
	}

	pub fn decode(encoded: &str) -> Result<usize, String> {
		let mut number = 0;
		for c in encoded.chars() {
			match ALPHABET.find(c) {
				Some(index) => {
					number = number * BASE + index;
				},
				None => return Err(format!("Invalid character '{}' found", c))
			}
		}
		Ok(number)
	}

