CREATE TABLE admins(
	reference_type TEXT NOT NULL CHECK ("reference_type" IN ('user', 'role')),
	reference_id NUMERIC NOT NULL,
	PRIMARY KEY (reference_type, reference_id)
);
