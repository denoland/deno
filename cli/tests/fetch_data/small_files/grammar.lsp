;;; -*- Mode: Lisp; Syntax: Common-Lisp; -*-

(define-language
  :grammar
  '(((S $any) -> (S1 $any))
    ((S (Compound $s1 $s2)) -> (S1 $s1) (Conjunction) (S1 $s2))
    
    ((S1 (Statement $v)) -> (NP $subj) (VP $subj $tense $v))
    ((S1 (Acknowledge $a)) -> (Acknowledge $a))
    ((S1 (Command $v)) -> (VP Self present $v))
    ((S1 (Question $v)) -> (Aux $tense) (NP $subj) (VP $subj $tense $v))
    ((S1 (Question $v)) -> (Be $tense) (NP $subj) (Be-Arg $subj $tense $v))

    ((Be-Arg $subj $tense (Occur $tense (loc $subj $loc))) ->
     (Loc-Adjunct $tense (loc $subj $loc)))

    ((VP $subj $tense (Occur $tense $v)) -> (VP1 $subj $tense $v))
    ((VP $subj $tense (Occur $tense $v)) -> (Aux $tense)(VP1 $subj present $v))

    ((VP1 $subj $tense $v) -> (VP2 $subj $tense $v) (Adjunct? $v))

    ((VP2 $subj $tense ($rel $subj $loc)) ->
     (Verb/in $rel $tense))
    ((VP2 $subj $tense ($rel $subj $loc $obj)) ->
     (Verb/tr $rel $tense) (NP $obj))
    ((VP2 $subj $tense ($rel $subj $loc $obj $obj2)) -> 
     (Verb/di $rel $tense) (NP $obj) (NP $obj2))
    ((VP2 $subj $tense (loc $subj $loc)) ->
     (Be $tense) (Loc-Adjunct $tense (loc $subj $loc)))

    ((NP $n) -> (Pronoun $n))
    ((NP $n) -> (Article) (Noun $n))
    ((NP $n) -> (Noun $n))
    ((NP ($x $y)) -> (Number $x) (Number $y))

    ((PP ($prep $n)) -> (Prep $prep) (NP $n))
    ((Adjunct? $v) ->)
    ((Adjunct? $v) -> (Loc-Adjunct $tense $v))
    #+Allegro ((Loc-Adjunct $tense ($rel $subj $loc @rest)) -> (PP $loc))
    #+Allegro ((Loc-Adjunct $tense ($rel $subj $loc @rest)) -> (Adjunct $loc))
    #+Lucid ((Loc-Adjunct $tense ($rel $subj $loc . $rest)) -> (PP $loc))
    #+Lucid ((Loc-Adjunct $tense ($rel $subj $loc . $rest)) -> (Adjunct $loc))

    )
  :lexicon
  '(
    ((Acknowledge $a) -> (yes true) (no false) (maybe unknown) (huh unparsed))
    ((Adjunct $loc) -> here there (nearby near) near left right up down)
    ((Article) -> a an the)
    ((Aux $tense) -> (will future) (did past) (do $finite))
    ((Be $tense) -> (am present) (are present) (is present) (be $finite)
     (was past) (were past))
    ((Conjunction) -> and --)
    ((Noun $n) -> gold Wumpus pit breeze stench glitter nothing)
    ((Number $n) -> 0 1 2 3 4 5 6 7 8 9)
    ((Prep $prep) -> in at to near)
    ((Pronoun $n) -> (you self) (me master) (I master))
    
    ((Verb/in $rel $tense) -> (go move $finite) (went move past)
     (move move $finite) (move move past) (shoot shoot $finite))
    ((Verb/tr $rel $tense) -> (move carry $finite) (moved carry past)
     (carry carry $finite) (carry carried past)
     (grab grab $finite) (grab grabbed past) (get grab $finite)
     (got grab past) (release release $finite) (release release past)
     (drop release $finite) (dropped release past) (shoot shoot-at $finite)
     (shot shoot-at past) (kill shoot-at $finite) (killed shoot-at past)
     (smell perceive $finite) (feel perceive $finite) (felt perceive past))
    ((Verb/di $rel $tense) -> (bring bring $finite) (brought bring past)
     (get bring $finite) (got bring past))
    ))

(defparameter *sentences*
  '((I will shoot the wumpus at 4 4)
    (yes)
    (You went right -- I will go left)
    (carry the gold)
    (yes and no)
    (did you bring me the gold)
    (a breeze is here -- I am near 5 3)
    (a stench is in 3 5)
    (a pit is nearby)
    (is the wumpus near)
    (Did you go to 3 8)
    (Yes -- Nothing is there)
    (Shoot -- Shoot left)
    (Kill the wumpus -- shoot up)))

(defun ss (&optional (sentences *sentences*))
  "Run some test sentences, and count how many were not parsed."
  (count-if-not
   #'(lambda (s)
       (format t "~2&>>> ~(~{~a ~}~)~%" s)
       (write (second (parse s)) :pretty t))
   *sentences*))
