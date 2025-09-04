SELECT
  *
FROM
  foo;

UPDATE
  foo
SET
  a = 'b'
WHERE
  id = 'biz';

CREATE TABLE foo(
  id text NOT NULL bar text,
  biz int,
  buz number NOT NULL
);

INSERT INTO
  user_data (first_name, last_name, address, phone, email)
VALUES
  ('foo', 'bar', 'biz', 1, 'bix');
