//! SQL Analyzer - Type inference from SQL queries based on Schema definitions
//!
//! Analyzes SQL template literals and infers result types based on the Schema.

use std::collections::HashMap;
use crate::ast::{SchemaDef, ColumnType, ColumnDef};

/// Analyzed result of a SQL query
#[derive(Debug, Clone)]
pub struct SqlQueryResult {
    /// Columns returned by the query
    pub columns: Vec<SqlResultColumn>,
    /// Whether the result can be null (e.g., .first() on empty result)
    pub nullable: bool,
    /// Whether the result is an array (e.g., .all() vs .first())
    pub is_array: bool,
}

/// A column in the query result
#[derive(Debug, Clone)]
pub struct SqlResultColumn {
    pub name: String,
    pub column_type: ColumnType,
    pub nullable: bool,
}

/// SQL Analyzer for type inference
pub struct SqlAnalyzer {
    /// Schema definitions indexed by table name
    tables: HashMap<String, Vec<ColumnDef>>,
}

impl SqlAnalyzer {
    /// Create a new analyzer from a Schema definition
    pub fn new(schema: &SchemaDef) -> Self {
        let mut tables = HashMap::new();
        for table in &schema.tables {
            tables.insert(table.name.clone(), table.columns.clone());
        }
        Self { tables }
    }

    /// Analyze a SQL query and infer its result type
    pub fn analyze(&self, sql: &str) -> Option<SqlQueryResult> {
        let sql_upper = sql.to_uppercase();
        let sql_trimmed = sql.trim();

        if sql_upper.starts_with("SELECT") {
            self.analyze_select(sql_trimmed)
        } else if sql_upper.starts_with("INSERT") {
            self.analyze_insert(sql_trimmed)
        } else if sql_upper.starts_with("UPDATE") {
            self.analyze_update(sql_trimmed)
        } else if sql_upper.starts_with("DELETE") {
            self.analyze_delete(sql_trimmed)
        } else {
            None
        }
    }

    /// Analyze SELECT query (supports JOIN)
    fn analyze_select(&self, sql: &str) -> Option<SqlQueryResult> {
        // SQL parser for SELECT queries with JOIN support
        // SELECT col1, col2 FROM table1 JOIN table2 ON ... WHERE ...

        let sql_upper = sql.to_uppercase();

        // Find FROM clause
        let from_pos = sql_upper.find(" FROM ")?;
        let after_from = &sql[from_pos + 6..];

        // Parse table references (including JOINs)
        let table_aliases = self.parse_table_references(after_from);

        // Extract SELECT columns
        let select_part = &sql[7..from_pos].trim();

        let result_columns = if *select_part == "*" {
            // SELECT * - return all columns from all tables
            let mut all_columns = Vec::new();
            for (table_name, alias) in &table_aliases {
                if let Some(columns) = self.tables.get(table_name) {
                    for col in columns {
                        let col_name = if table_aliases.len() > 1 {
                            // Prefix with alias for JOINs
                            format!("{}_{}", alias.as_deref().unwrap_or(table_name), col.name)
                        } else {
                            col.name.clone()
                        };
                        all_columns.push(SqlResultColumn {
                            name: col_name,
                            column_type: col.column_type.clone(),
                            nullable: col.nullable,
                        });
                    }
                }
            }
            all_columns
        } else {
            // Parse individual columns
            let mut result = Vec::new();
            for col_expr in select_part.split(',') {
                let col_expr = col_expr.trim();

                // Handle alias: "column AS alias" or "column alias"
                let (col_ref, alias) = self.parse_column_alias(col_expr);

                // Handle table.column format
                let (table_ref, col_name) = if col_ref.contains('.') {
                    let parts: Vec<&str> = col_ref.splitn(2, '.').collect();
                    (Some(parts[0]), parts[1])
                } else {
                    (None, col_ref)
                };

                // Find column in schema
                let col_def = self.find_column(&table_aliases, table_ref, col_name);

                if let Some((found_col, is_join)) = col_def {
                    result.push(SqlResultColumn {
                        name: alias.unwrap_or_else(|| col_name.to_string()),
                        column_type: found_col.column_type.clone(),
                        // LEFT/RIGHT JOIN columns are nullable
                        nullable: found_col.nullable || is_join,
                    });
                } else {
                    // Column might be an expression or function
                    result.push(SqlResultColumn {
                        name: alias.unwrap_or_else(|| col_name.to_string()),
                        column_type: ColumnType::String,
                        nullable: true,
                    });
                }
            }
            result
        };

        Some(SqlQueryResult {
            columns: result_columns,
            nullable: true,
            is_array: true,
        })
    }

    /// Parse table references from FROM clause (including JOINs)
    /// Returns: Vec<(table_name, optional_alias)>
    fn parse_table_references(&self, from_clause: &str) -> Vec<(String, Option<String>)> {
        let mut tables = Vec::new();
        let upper = from_clause.to_uppercase();

        // Split by JOIN keywords
        let join_keywords = [" LEFT JOIN ", " RIGHT JOIN ", " INNER JOIN ", " JOIN ", " OUTER JOIN ", " CROSS JOIN "];

        // Find the end of FROM clause (WHERE, ORDER BY, GROUP BY, LIMIT, etc.)
        let end_keywords = [" WHERE ", " ORDER ", " GROUP ", " LIMIT ", " HAVING "];
        let end_pos = end_keywords.iter()
            .filter_map(|kw| upper.find(kw))
            .min()
            .unwrap_or(from_clause.len());

        let from_part = &from_clause[..end_pos];

        // Check if there are JOINs
        let has_join = join_keywords.iter().any(|kw| upper.contains(kw));

        if !has_join {
            // Simple FROM table [AS alias]
            let table_ref = self.parse_single_table_ref(from_part);
            if let Some(t) = table_ref {
                tables.push(t);
            }
        } else {
            // Parse JOINs
            let mut remaining = from_part.to_string();
            let remaining_upper = remaining.to_uppercase();

            // Find first table (before any JOIN)
            let first_join_pos = join_keywords.iter()
                .filter_map(|kw| remaining_upper.find(kw))
                .min()
                .unwrap_or(remaining.len());

            let first_table = &remaining[..first_join_pos];
            if let Some(t) = self.parse_single_table_ref(first_table) {
                tables.push(t);
            }

            // Parse each JOIN
            remaining = remaining[first_join_pos..].to_string();

            while !remaining.is_empty() {
                let remaining_upper = remaining.to_uppercase();

                // Find current JOIN type
                let mut join_start = None;
                for kw in &join_keywords {
                    if remaining_upper.starts_with(kw.trim_start()) {
                        join_start = Some(kw.trim_start().len());
                        break;
                    }
                }

                if join_start.is_none() {
                    break;
                }

                let after_join = &remaining[join_start.unwrap()..];

                // Find ON clause or next JOIN
                let on_pos = after_join.to_uppercase().find(" ON ");
                let next_join_pos = join_keywords.iter()
                    .filter_map(|kw| after_join.to_uppercase().find(kw))
                    .min();

                let table_end = match (on_pos, next_join_pos) {
                    (Some(on), Some(next)) => on.min(next),
                    (Some(on), None) => on,
                    (None, Some(next)) => next,
                    (None, None) => after_join.len(),
                };

                let table_ref = &after_join[..table_end];
                if let Some(t) = self.parse_single_table_ref(table_ref) {
                    tables.push(t);
                }

                // Move past ON clause if present
                if let Some(on) = on_pos {
                    let after_on = &after_join[on + 4..];
                    // Find next JOIN or end
                    let next = join_keywords.iter()
                        .filter_map(|kw| after_on.to_uppercase().find(kw))
                        .min()
                        .unwrap_or(after_on.len());
                    remaining = after_on[next..].to_string();
                } else {
                    remaining = if let Some(next) = next_join_pos {
                        after_join[next..].to_string()
                    } else {
                        String::new()
                    };
                }
            }
        }

        tables
    }

    /// Parse a single table reference: "table_name [AS] alias" or just "table_name"
    fn parse_single_table_ref(&self, s: &str) -> Option<(String, Option<String>)> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let upper = s.to_uppercase();

        // Check for AS keyword
        if let Some(as_pos) = upper.find(" AS ") {
            let table = s[..as_pos].trim().to_string();
            let alias = s[as_pos + 4..].trim().to_string();
            Some((table, Some(alias)))
        } else {
            // Check for implicit alias (table_name alias)
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                Some((parts[0].to_string(), Some(parts[1].to_string())))
            } else if parts.len() == 1 {
                Some((parts[0].to_string(), None))
            } else {
                None
            }
        }
    }

    /// Parse column alias: "column AS alias" -> (column, Some(alias))
    fn parse_column_alias<'a>(&self, col_expr: &'a str) -> (&'a str, Option<String>) {
        let upper = col_expr.to_uppercase();

        if let Some(as_pos) = upper.find(" AS ") {
            let col = col_expr[..as_pos].trim();
            let alias = col_expr[as_pos + 4..].trim().to_string();
            (col, Some(alias))
        } else {
            (col_expr.trim(), None)
        }
    }

    /// Find a column in the schema, considering table aliases
    fn find_column(
        &self,
        table_aliases: &[(String, Option<String>)],
        table_ref: Option<&str>,
        col_name: &str,
    ) -> Option<(ColumnDef, bool)> {
        // If table reference is specified, look in that table
        if let Some(ref_name) = table_ref {
            for (table_name, alias) in table_aliases {
                if table_name == ref_name || alias.as_deref() == Some(ref_name) {
                    if let Some(columns) = self.tables.get(table_name) {
                        if let Some(col) = columns.iter().find(|c| c.name == col_name) {
                            let is_join = table_aliases.len() > 1;
                            return Some((col.clone(), is_join));
                        }
                    }
                }
            }
        } else {
            // Search all tables
            for (table_name, _) in table_aliases {
                if let Some(columns) = self.tables.get(table_name) {
                    if let Some(col) = columns.iter().find(|c| c.name == col_name) {
                        let is_join = table_aliases.len() > 1;
                        return Some((col.clone(), is_join));
                    }
                }
            }
        }
        None
    }

    /// Analyze INSERT query - returns inserted row
    fn analyze_insert(&self, sql: &str) -> Option<SqlQueryResult> {
        let sql_upper = sql.to_uppercase();

        // INSERT INTO table_name ...
        let into_pos = sql_upper.find(" INTO ")?;
        let after_into = &sql[into_pos + 6..];

        let table_name = after_into
            .split(|c: char| c.is_whitespace() || c == '(')
            .next()?
            .trim();

        let columns = self.tables.get(table_name)?;

        // INSERT returns the inserted row
        let result_columns = columns
            .iter()
            .map(|col| SqlResultColumn {
                name: col.name.clone(),
                column_type: col.column_type.clone(),
                nullable: col.nullable,
            })
            .collect();

        Some(SqlQueryResult {
            columns: result_columns,
            nullable: false, // INSERT returns the inserted row
            is_array: false,
        })
    }

    /// Analyze UPDATE query - returns affected count or updated rows
    fn analyze_update(&self, _sql: &str) -> Option<SqlQueryResult> {
        // UPDATE typically returns count or nothing
        Some(SqlQueryResult {
            columns: vec![SqlResultColumn {
                name: "changes".to_string(),
                column_type: ColumnType::Number,
                nullable: false,
            }],
            nullable: false,
            is_array: false,
        })
    }

    /// Analyze DELETE query - returns affected count
    fn analyze_delete(&self, _sql: &str) -> Option<SqlQueryResult> {
        Some(SqlQueryResult {
            columns: vec![SqlResultColumn {
                name: "changes".to_string(),
                column_type: ColumnType::Number,
                nullable: false,
            }],
            nullable: false,
            is_array: false,
        })
    }

    /// Get TypeScript type for a column type
    pub fn column_type_to_ts(col_type: &ColumnType) -> &'static str {
        match col_type {
            ColumnType::String => "string",
            ColumnType::Number => "number",
            ColumnType::Boolean => "boolean",
            ColumnType::Datetime => "Date",
            ColumnType::Json => "unknown",
            ColumnType::Blob => "Uint8Array",
        }
    }

    /// Generate TypeScript interface for query result
    pub fn generate_ts_type(&self, result: &SqlQueryResult) -> String {
        if result.columns.is_empty() {
            return "void".to_string();
        }

        let mut fields = Vec::new();
        for col in &result.columns {
            let ts_type = Self::column_type_to_ts(&col.column_type);
            let optional = if col.nullable { "?" } else { "" };
            fields.push(format!("  {}{}: {}", col.name, optional, ts_type));
        }

        let obj_type = format!("{{\n{}\n}}", fields.join(";\n"));

        if result.is_array {
            format!("{}[]", obj_type)
        } else if result.nullable {
            format!("{} | null", obj_type)
        } else {
            obj_type
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{TableDef, ColumnConstraint};

    fn create_test_schema() -> SchemaDef {
        SchemaDef {
            tables: vec![
                TableDef {
                    name: "users".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            column_type: ColumnType::String,
                            nullable: false,
                            constraints: vec![ColumnConstraint::Primary],
                        },
                        ColumnDef {
                            name: "name".to_string(),
                            column_type: ColumnType::String,
                            nullable: false,
                            constraints: vec![],
                        },
                        ColumnDef {
                            name: "email".to_string(),
                            column_type: ColumnType::String,
                            nullable: false,
                            constraints: vec![ColumnConstraint::Unique],
                        },
                        ColumnDef {
                            name: "age".to_string(),
                            column_type: ColumnType::Number,
                            nullable: true,
                            constraints: vec![],
                        },
                    ],
                },
                TableDef {
                    name: "posts".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            column_type: ColumnType::String,
                            nullable: false,
                            constraints: vec![ColumnConstraint::Primary],
                        },
                        ColumnDef {
                            name: "user_id".to_string(),
                            column_type: ColumnType::String,
                            nullable: false,
                            constraints: vec![],
                        },
                        ColumnDef {
                            name: "title".to_string(),
                            column_type: ColumnType::String,
                            nullable: false,
                            constraints: vec![],
                        },
                        ColumnDef {
                            name: "content".to_string(),
                            column_type: ColumnType::String,
                            nullable: true,
                            constraints: vec![],
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn test_analyze_select_all() {
        let schema = create_test_schema();
        let analyzer = SqlAnalyzer::new(&schema);

        let result = analyzer.analyze("SELECT * FROM users").unwrap();
        assert_eq!(result.columns.len(), 4);
        assert!(result.is_array);
    }

    #[test]
    fn test_analyze_select_specific_columns() {
        let schema = create_test_schema();
        let analyzer = SqlAnalyzer::new(&schema);

        let result = analyzer.analyze("SELECT id, name FROM users WHERE id = ?").unwrap();
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "name");
    }

    #[test]
    fn test_generate_ts_type() {
        let schema = create_test_schema();
        let analyzer = SqlAnalyzer::new(&schema);

        let result = analyzer.analyze("SELECT id, name, age FROM users").unwrap();
        let ts_type = analyzer.generate_ts_type(&result);

        assert!(ts_type.contains("id: string"));
        assert!(ts_type.contains("name: string"));
        assert!(ts_type.contains("age?: number")); // nullable
    }

    #[test]
    fn test_analyze_join() {
        let schema = create_test_schema();
        let analyzer = SqlAnalyzer::new(&schema);

        let result = analyzer.analyze(
            "SELECT u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id"
        ).unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "name");
        assert_eq!(result.columns[1].name, "title");
    }

    #[test]
    fn test_analyze_left_join() {
        let schema = create_test_schema();
        let analyzer = SqlAnalyzer::new(&schema);

        let result = analyzer.analyze(
            "SELECT users.name, posts.title FROM users LEFT JOIN posts ON users.id = posts.user_id"
        ).unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "name");
        assert_eq!(result.columns[1].name, "title");
    }

    #[test]
    fn test_analyze_join_with_alias() {
        let schema = create_test_schema();
        let analyzer = SqlAnalyzer::new(&schema);

        let result = analyzer.analyze(
            "SELECT u.name AS author_name, p.title AS post_title FROM users AS u JOIN posts AS p ON u.id = p.user_id"
        ).unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0].name, "author_name");
        assert_eq!(result.columns[1].name, "post_title");
    }
}
