// Copyright (c) 2023-2025 ParadeDB, Inc.
//
// This file is part of ParadeDB - Postgres for Search and Analytics
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use unicode_normalization::UnicodeNormalization;

use once_cell::sync::Lazy;
use std::sync::Arc;

use charabia::normalizer::NormalizedTokenIter;
use charabia::{Tokenizer, TokenizerBuilder};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer as TantivyTokenizer};

static CHARABIA_TOKENIZER: Lazy<Arc<Tokenizer>> = Lazy::new(|| {
    Arc::new(TokenizerBuilder::default().into_tokenizer())
});

#[derive(Clone, Default)]
pub struct CharabiaTokenizer;

impl TantivyTokenizer for CharabiaTokenizer {
    type TokenStream<'a> = CharabiaTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        CharabiaTokenStream::new(text)
    }
}

pub struct CharabiaTokenStream<'a> {
    tokens: NormalizedTokenIter<'a, 'static, 'static, 'static>,
    token: Token,
    position: usize,
}

impl<'a> CharabiaTokenStream<'a> {
    pub fn new(text: &'a str) -> Self {
        let tokens = CHARABIA_TOKENIZER.tokenize(text);

        Self {
            tokens,
            token: Token::default(),
            position: 0,
        }
    }
}

impl<'a> TokenStream for CharabiaTokenStream<'a> {
    fn advance(&mut self) -> bool {
        while let Some(token) = self.tokens.next() {
            if token.is_word() {
                self.token.text = token.lemma().nfc().collect::<String>();
                self.token.offset_from = token.byte_start;
                self.token.offset_to = token.byte_end;
                self.token.position = self.position;
                self.position += 1;
                return true;
            }
        }
        false
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use tantivy::tokenizer::{Token, TokenStream, Language};
    use crate::manager::{SearchTokenizer, SearchTokenizerFilters};

    fn test_helper<T: TantivyTokenizer>(tokenizer: &mut T, text: &str) -> Vec<Token> {
        let mut token_stream = tokenizer.token_stream(text);
        let mut tokens: Vec<Token> = vec![];
        while token_stream.advance() {
            tokens.push(token_stream.token().clone());
        }
        tokens
    }

    #[rstest]
    fn test_charabia_tokenizer() {
        let mut tokenizer = CharabiaTokenizer::default();
        let tokens = test_helper(
            &mut tokenizer,
            "地址1，包含無效的字元 (包括符號與不標準的asci阿爾發字元",
        );

        assert_eq!(tokens.len(), 22);
        {
            let token = &tokens[0];
            assert_eq!(token.text, "地址");
            assert_eq!(token.offset_from, 0);
            assert_eq!(token.offset_to, 6);
            assert_eq!(token.position, 0);
            assert_eq!(token.position_length, 1);
        }
        {
            let token = &tokens[1];
            assert_eq!(token.text, "1");
            assert_eq!(token.offset_from, 6);
            assert_eq!(token.offset_to, 7);
            assert_eq!(token.position, 1);
            assert_eq!(token.position_length, 1);
        }
    }

    #[rstest]
    fn test_charabia_tokenizer_japanese() {
        let mut tokenizer = CharabiaTokenizer::default();
        let tokens = test_helper(&mut tokenizer, "すもももももももものうち");
        assert_eq!(tokens.len(), 7);
        {
            let token = &tokens[0];
            assert_eq!(token.text, "すもも");
            assert_eq!(token.offset_from, 0);
            assert_eq!(token.offset_to, 9);
            assert_eq!(token.position, 0);
            assert_eq!(token.position_length, 1);
        }
        {
            let token = &tokens[1];
            assert_eq!(token.text, "も");
            assert_eq!(token.offset_from, 9);
            assert_eq!(token.offset_to, 12);
            assert_eq!(token.position, 1);
            assert_eq!(token.position_length, 1);
        }
    }

    #[rstest]
    fn test_charabia_tokenizer_korean() {
        let mut tokenizer = CharabiaTokenizer::default();
        let tokens = test_helper(&mut tokenizer, "일본입니다. 매우 멋진 단어입니다.");
        assert_eq!(tokens.len(), 6);
        {
            let token = &tokens[0];
            assert_eq!(token.text, "일본");
            assert_eq!(token.offset_from, 0);
            assert_eq!(token.offset_to, 6);
            assert_eq!(token.position, 0);
            assert_eq!(token.position_length, 1);
        }
        {
            let token = &tokens[1];
            assert_eq!(token.text, "입니다");
            assert_eq!(token.offset_from, 6);
            assert_eq!(token.offset_to, 15);
            assert_eq!(token.position, 1);
            assert_eq!(token.position_length, 1);
        }
    }

    #[rstest]
    fn test_charabia_tokenizer_with_empty_string() {
        let mut tokenizer = CharabiaTokenizer::default();
        {
            let tokens = test_helper(&mut tokenizer, "");
            assert_eq!(tokens.len(), 0);
        }
        {
            let tokens = test_helper(&mut tokenizer, "    ");
            assert_eq!(tokens.len(), 0);
        }
    }

    #[rstest]
    fn test_charabia_tokenizer_with_stopwords_language() {
        let search_tokenizer = SearchTokenizer::Charabia(SearchTokenizerFilters {
            stopwords_language: Some(Language::English),
            ..Default::default()
        });
        let mut tokenizer = search_tokenizer.to_tantivy_tokenizer().unwrap();
        let mut tokens = Vec::new();
        let mut token_stream = tokenizer.token_stream("the quick brown fox jumps over the lazy dog");
        while let Some(token) = token_stream.next() {
            tokens.push(token.clone());
        }
        let expected_tokens: Vec<String> = vec!["quick", "brown", "fox", "jumps", "over", "lazy", "dog"].into_iter().map(|s| s.to_string()).collect();
        let actual_tokens: Vec<String> = tokens.iter().map(|t| t.text.clone()).collect();
        assert_eq!(actual_tokens, expected_tokens);
    }

    #[rstest]
    fn test_charabia_tokenizer_with_custom_stopwords() {
        let search_tokenizer = SearchTokenizer::Charabia(SearchTokenizerFilters {
            stopwords: Some(vec!["fox".to_string(), "dog".to_string()]),
            ..Default::default()
        });
        let mut tokenizer = search_tokenizer.to_tantivy_tokenizer().unwrap();
        let mut tokens = Vec::new();
        let mut token_stream = tokenizer.token_stream("the quick brown fox jumps over the lazy dog");
        while let Some(token) = token_stream.next() {
            tokens.push(token.clone());
        }
        let expected_tokens: Vec<String> = vec!["the", "quick", "brown", "jumps", "over", "the", "lazy"].into_iter().map(|s| s.to_string()).collect();
        let actual_tokens: Vec<String> = tokens.iter().map(|t| t.text.clone()).collect();
        assert_eq!(actual_tokens, expected_tokens);
    }
}