//! Quiz state shared by the terminal and desktop quiz screens.

use chrono::Utc;

use crate::model::{Mcq, QuizAttempt};

#[derive(Debug, Clone)]
pub struct QuizSession {
    questions: Vec<Mcq>,
    answers: Vec<Option<usize>>,
    current: usize,
}

/// Feedback for one answered question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub correct: bool,
    pub chosen: usize,
    pub answer_index: usize,
}

impl QuizSession {
    pub fn new(questions: Vec<Mcq>) -> Self {
        let answers = vec![None; questions.len()];
        Self {
            questions,
            answers,
            current: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.questions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.questions.is_empty()
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn current(&self) -> Option<&Mcq> {
        self.questions.get(self.current)
    }

    /// Answer for the current question, if one was given.
    pub fn current_answer(&self) -> Option<usize> {
        self.answers.get(self.current).copied().flatten()
    }

    /// Records an answer for the current question. Answers are final.
    pub fn answer(&mut self, choice: usize) -> Option<Feedback> {
        let q = self.questions.get(self.current)?;
        if choice >= q.options.len() {
            return None;
        }
        let slot = &mut self.answers[self.current];
        let chosen = *slot.get_or_insert(choice);
        Some(Feedback {
            correct: chosen == q.answer_index,
            chosen,
            answer_index: q.answer_index,
        })
    }

    /// Moves to the next question. Returns false when the quiz is over.
    pub fn advance(&mut self) -> bool {
        if self.current < self.questions.len() {
            self.current += 1;
        }
        self.current < self.questions.len()
    }

    pub fn is_finished(&self) -> bool {
        self.current >= self.questions.len()
    }

    pub fn score(&self) -> usize {
        self.questions
            .iter()
            .zip(&self.answers)
            .filter(|(q, a)| **a == Some(q.answer_index))
            .count()
    }

    pub fn attempt(&self) -> QuizAttempt {
        QuizAttempt {
            at: Utc::now(),
            score: self.score(),
            total: self.questions.len(),
        }
    }

    /// Questions answered wrong, for a review pass.
    pub fn missed(&self) -> Vec<&Mcq> {
        self.questions
            .iter()
            .zip(&self.answers)
            .filter(|(q, a)| **a != Some(q.answer_index))
            .map(|(q, _)| q)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(answer: usize) -> Mcq {
        Mcq {
            question: format!("q{answer}"),
            options: vec!["a".into(), "b".into(), "c".into()],
            answer_index: answer,
            explanation: String::new(),
        }
    }

    #[test]
    fn scores_a_full_run() {
        let mut s = QuizSession::new(vec![q(0), q(2), q(1)]);
        assert!(s.answer(0).unwrap().correct);
        assert!(s.advance());
        let fb = s.answer(1).unwrap();
        assert!(!fb.correct);
        assert_eq!(fb.answer_index, 2);
        assert!(s.advance());
        assert!(s.answer(1).unwrap().correct);
        assert!(!s.advance());
        assert!(s.is_finished());
        assert_eq!(s.score(), 2);
        assert_eq!(s.attempt().total, 3);
        assert_eq!(s.missed().len(), 1);
        assert_eq!(s.missed()[0].question, "q2");
    }

    #[test]
    fn first_answer_sticks_and_bad_choices_are_ignored() {
        let mut s = QuizSession::new(vec![q(1)]);
        assert!(s.answer(9).is_none());
        assert!(!s.answer(0).unwrap().correct);
        // Changing your mind does not change the recorded answer.
        assert_eq!(s.answer(1).unwrap().chosen, 0);
        assert_eq!(s.current_answer(), Some(0));
        assert!(!s.advance());
        assert!(s.answer(0).is_none());
        assert!(!s.advance());
    }
}
