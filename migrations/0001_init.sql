CREATE TABLE admins(
	"type" TEXT NOT NULL CHECK ("type" IN ('user', 'role')),
	"id" BIGINT NOT NULL,
	PRIMARY KEY ("type", "id")
);
