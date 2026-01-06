SHELL := /bin/bash
PROJECT_DIR := $(abspath $(CURDIR))
FRONT_DIR := ${PROJECT_DIR}/app/frontend
INFRA_DIR := ${PROJECT_DIR}/app/infra
BACKEND_DIR := ${PROJECT_DIR}/app/server
ENV_PATH := PATH="$(HOME)/.local/bin:$(HOME)/.cargo/bin:$$PATH"

# workers を make setup w=6 の w で受ける例
w ?= 3

setup:
	.claude/scripts/setup.sh $(w)

# 再インデックス（VSCodeの最初の解析に相当）
reindex:
	uvx --from git+https://github.com/oraios/serena \
		serena project index

# リトライ
msg ?= "あなたはpresidentです。指示書に従って"
resume:
	.claude/scripts/send-at-time.sh ${at} ${msg}

ri:
	sudo rm -rf ${FRONT_DIR}/node_modules 
	sudo rm -rf ${FRONT_DIR}/.next
	sudo rm -rf ${FRONT_DIR}/package-lock.json
	npm i --prefix ${FRONT_DIR}

r:
	sudo rm -rf ${FRONT_DIR}/.next
	
i:
	　npm i --prefix ${FRONT_DIR}

# モック起動
dev:
	npm run dev --prefix ${FRONT_DIR}

# モック起動
build:
	npm run build:dev --prefix ${FRONT_DIR}


front-reset:
	rm -rf app/front/
	
claude:
	claude --dangerously-skip-permissions

ii:
	npm i --prefix ${INFRA_DIR}

lint:
	npm run lint --prefix ${INFRA_DIR}

tsc: 
	npm run tsc --prefix ${INFRA_DIR}

# Docker環境管理
APP_DIR := ${PROJECT_DIR}/app

# Docker起動（Mock モード - デフォルト）
up:
	cd ${APP_DIR} && docker compose up -d

# Docker起動（Mock モード - 明示的）
up-mock:
	cd ${APP_DIR} && docker compose --env-file .env.mock up -d

# Docker起動（Dev モード）
up-dev:
	cd ${APP_DIR} && docker compose --env-file .env.dev up -d

# Docker停止
down:
	cd ${APP_DIR} && docker compose down

# Docker完全停止（ボリューム削除）
down-v:
	cd ${APP_DIR} && docker compose down -v

# Docker再起動
restart:
	cd ${APP_DIR} && docker compose restart

# Docker再起動（Mock モード）
restart-mock:
	cd ${APP_DIR} && docker compose down && docker compose --env-file .env.mock up -d

# Docker再起動（Dev モード）
restart-dev:
	cd ${APP_DIR} && docker compose down && docker compose --env-file .env.dev up -d

# Dockerログ確認
docker-logs:
	cd ${APP_DIR} && docker compose logs -f

# 特定サービスのログ確認（例: make logs-service s=localstack）
s ?= localstack
logs-service:
	cd ${APP_DIR} && docker compose logs -f ${s}

# LocalStackのステータス確認
status:
	cd ${APP_DIR} && docker compose ps

# LocalStack内のAWSリソース確認
aws-status:
	cd ${APP_DIR} && docker compose exec localstack awslocal s3 ls
	cd ${APP_DIR} && docker compose exec localstack awslocal lambda list-functions --query 'Functions[].FunctionName' --output table
	cd ${APP_DIR} && docker compose exec localstack awslocal sqs list-queues --query 'QueueUrls' --output table

# Lambda関数のテスト実行
test-lambda:
	cd ${APP_DIR} && docker compose exec localstack awslocal lambda invoke \
		--function-name document-search-api \
		--payload '{"path": "/health", "httpMethod": "GET"}' \
		/tmp/response.json && \
	docker compose exec localstack cat /tmp/response.json

# S3ファイルの確認
s3-list:
	cd ${APP_DIR} && docker compose exec localstack awslocal s3 ls s3://document-storage --recursive

# 環境のクリーンアップ
clean:
	cd ${APP_DIR} && docker compose down -v --remove-orphans
	docker system prune -f

# Orphanコンテナの削除
clean-orphans:
	cd ${APP_DIR} && docker compose down --remove-orphans

# LocalStack初期化
init:
	@echo "🚀 Initializing LocalStack..."
	docker compose -f ${APP_DIR}/docker-compose.yml exec localstack bash -c "cd /docker-entrypoint-initaws.d && ./00-setup.sh"
	docker compose -f ${APP_DIR}/docker-compose.yml exec localstack bash -c "cd /docker-entrypoint-initaws.d && ./01-init-s3.sh"
	docker compose -f ${APP_DIR}/docker-compose.yml exec localstack bash -c "cd /docker-entrypoint-initaws.d && ./02-build-lambdas.sh"
	docker compose -f ${APP_DIR}/docker-compose.yml exec localstack bash -c "cd /docker-entrypoint-initaws.d && ./03-deploy-lambdas.sh"
	@echo "✅ LocalStack initialization completed!"

# 完全セットアップ（起動＋初期化）
setup-full:
	@echo "🏗️ Starting full setup..."
	cd ${APP_DIR} && docker compose up -d localstack
	@echo "⏳ Waiting for LocalStack to be ready..."
	@sleep 15
	@make init
	@echo "🎉 Full setup completed!"

# ヘルプ
help:
	@echo "Docker環境管理コマンド:"
	@echo "  up          - Docker起動（Mock モード）"
	@echo "  up-mock     - Docker起動（Mock モード明示的）"
	@echo "  up-dev      - Docker起動（Dev モード）"
	@echo "  down        - Docker停止"
	@echo "  down-v      - Docker停止（ボリューム削除）"
	@echo "  restart     - Docker再起動"
	@echo "  restart-mock - Docker再起動（Mock モード）"
	@echo "  restart-dev - Docker再起動（Dev モード）"
	@echo "  logs        - ログ確認"
	@echo "  logs-service s=<service> - 特定サービスのログ確認"
	@echo "  status      - コンテナステータス確認"
	@echo "  aws-status  - AWSリソースステータス確認"
	@echo "  test-lambda - Lambda関数テスト実行"
	@echo "  s3-list     - S3ファイル一覧"
	@echo "  clean       - 環境クリーンアップ"
	@echo ""
	@echo "使用例:"
	@echo "  make up              # Mock モードで起動"
	@echo "  make up-dev          # Dev モードで起動"
	@echo "  make logs-service s=frontend  # フロントエンドのログ確認"

