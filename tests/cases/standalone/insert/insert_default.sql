CREATE TABLE test1 (i INTEGER, j BIGINT TIME INDEX, k STRING DEFAULT 'blabla');

INSERT INTO test1 VALUES (DEFAULT);

INSERT INTO test1 VALUES (DEFAULT, DEFAULT, DEFAULT);

INSERT INTO test1 VALUES (DEFAULT, DEFAULT, DEFAULT, DEFAULT);

INSERT INTO test1 VALUES (DEFAULT, 1, DEFAULT), (default, 2, default), (DeFaUlT, 3, DeFaUlT), (dEfAuLt, 4, dEfAuLt);

SELECT * FROM test1;

CREATE TABLE test2 (i INTEGER, j BIGINT TIME INDEX DEFAULT CURRENT_TIMESTAMP, k STRING DEFAULT 'blabla');

INSERT INTO test2 VALUES (1,1,'a'), (default, 2, default), (3,3,'b'), (default, 4, default), (5, 5, 'c');

INSERT INTO test2 VALUES (6, 6, default), (7, 7, 'd'), (default, 8, 'e');

SELECT * FROM test2;

DROP TABLE test1;
DROP TABLE test2;
