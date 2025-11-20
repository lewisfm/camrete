//! (De)serializing method which represents no items as null,
//! one item as that item itself, and many items as a list.


use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
};

pub fn deserialize<'a, T, D>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'a>,
    T: Deserialize<'a>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NullOrItemOrListOfItems<T> {
        Item(Option<T>),
        List(Vec<T>),
    }

    let repr = NullOrItemOrListOfItems::<T>::deserialize(d)?;

    Ok(match repr {
        NullOrItemOrListOfItems::Item(item) => item.into_iter().collect(),
        NullOrItemOrListOfItems::List(list) => list,
    })
}

pub fn serialize<T, S>(value: &[T], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    #[derive(Serialize)]
    #[serde(untagged)]
    enum NullOrItemOrListOfItems<'a, T: Serialize> {
        Item(Option<&'a T>),
        List(&'a [T]),
    }

    let repr = if value.len() >= 2 {
        NullOrItemOrListOfItems::List(value)
    } else {
        NullOrItemOrListOfItems::Item(value.first())
    };

    repr.serialize(s)
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};
    use serde_test::{assert_tokens, Token};

    #[derive(Debug, PartialEq, Eq)]
    struct Test<T: Serialize>(Vec<T>);

    impl<T: Serialize> Serialize for Test<T> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer {
            super::serialize(&self.0, serializer)
        }
    }

    impl<'a, T: Serialize + Deserialize<'a>> Deserialize<'a> for Test<T> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'a> {
            super::deserialize(deserializer).map(Self)
        }
    }

    #[test]
    fn ser_de_none() {
        let value = Test(Vec::<String>::new());

        assert_tokens(&value, &[
            Token::None,
        ]);
    }

    #[test]
    fn ser_de_single() {
        let value = Test(vec!["hi".to_string()]);

        assert_tokens(&value, &[
            Token::Some,
            Token::Str("hi"),
        ]);
    }

    #[test]
    fn ser_de_many() {
        let value = Test(vec!["hi".to_string(), "there".to_string()]);

        assert_tokens(&value, &[
            Token::Seq { len: Some(2) },
            Token::Str("hi"),
            Token::Str("there"),
            Token::SeqEnd,
        ]);
    }
}
