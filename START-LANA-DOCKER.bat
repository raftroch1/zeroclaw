@echo off
echo ========================================
echo Starting Lana (ZeroClaw) in Docker
echo ========================================
echo.

REM Check if Docker is running
docker info >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Docker is not running!
    echo Please start Docker Desktop first.
    pause
    exit /b 1
)

echo Building and starting ZeroClaw container...
echo.

REM Build and start
docker compose -f docker-compose.lana.yml up -d --build

if %errorlevel% neq 0 (
    echo.
    echo ERROR: Failed to start container!
    echo Check the error message above.
    pause
    exit /b 1
)

echo.
echo ========================================
echo Lana is starting in Docker!
echo ========================================
echo.
echo View logs: docker compose -f docker-compose.lana.yml logs -f
echo Stop: docker compose -f docker-compose.lana.yml down
echo Restart: docker compose -f docker-compose.lana.yml restart
echo.
echo Container will auto-start on boot (restart: unless-stopped)
echo.
pause
