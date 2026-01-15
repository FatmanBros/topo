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

    /// Analyze SELECT query
    fn analyze_select(&self, sql: &str) -> Option<SqlQueryResult> {
        // Simple SQL parser for SELECT queries
        // SELECT col1, col2 FROM table_name WHERE ...

        let sql_upper = sql.to_uppercase();

        // Find FROM clause to get table name
        let from_pos = sql_upper.find(" FROM ")?;
        let after_from = &sql[from_pos + 6..];

        // Extract table name (first word after FROM)
        let table_name = after_from
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

        let columns = self.tables.get(table_name)?;

        // Extract SELECT columns
        let select_part = &sql[7..from_pos].trim();

        let result_columns = if *select_part == "*" {
            // SELECT * - return all columns
            columns
                .iter()
                .map(|col| SqlResultColumn {
                    name: col.name.clone(),
                    column_type: col.column_type.clone(),
                    nullable: col.nullable,
                })
                .collect()
        } else {
            // Parse individual columns
            let mut result = Vec::new();
            for col_expr in select_part.split(',') {
                let col_name = col_expr.trim().split_whitespace().next()?;
                // Remove any alias (AS ...)
                let col_name = col_name.split(" AS ").next()?.trim();
                let col_name = col_name.split(" as ").next()?.trim();

                // Find column in schema
                if let Some(col_def) = columns.iter().find(|c| c.name == col_name) {
                    result.push(SqlResultColumn {
                        name: col_def.name.clone(),
                        column_type: col_def.column_type.clone(),
                        nullable: col_def.nullable,
                    });
                } else {
                    // Column might be an expression or function - treat as any
                    result.push(SqlResultColumn {
                        name: col_name.to_string(),
                        column_type: ColumnType::String, // Default to string
                        nullable: true,
                    });
                }
            }
            result
        };

        Some(SqlQueryResult {
            columns: result_columns,
            nullable: true, // SELECT can return null if no rows
            is_array: true, // Default to array, .first() makes it single
        })
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
}
