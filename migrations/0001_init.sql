CREATE TABLE admins(
	reference_type TEXT NOT NULL CHECK ("type" IN ('user', 'role')),
	reference_id NUMERIC NOT NULL,
	PRIMARY KEY (reference_type, refernce_id)
);
