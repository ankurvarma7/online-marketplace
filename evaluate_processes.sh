#!/bin/bash

################################################################################
# PROCESS EVALUATION SCRIPT
#
# This script evaluates running processes on deployed VMs and provides
# detailed status reports for troubleshooting and monitoring
#
# Usage: ./evaluate_vm_processes.sh [instance-name]
################################################################################

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
PROJECT_ID="online-marketplace-486000"
ZONE="us-central1-a"

# Instance configuration
INSTANCES=("customer-db" "product-db" "seller-server" "buyer-server" "seller-client" "buyer-client" "evaluator")

# Port mapping using simple bash
get_service_port() {
    case $1 in
        customer-db) echo "8080" ;;
        product-db) echo "8081" ;;
        seller-server) echo "8082" ;;
        buyer-server) echo "8083" ;;
        *) echo "none" ;;
    esac
}

################################################################################
# UTILITY FUNCTIONS
################################################################################

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

log_header() {
    echo ""
    echo -e "${CYAN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC} $1"
    echo -e "${CYAN}╚════════════════════════════════════════════════════════════╝${NC}"
}

separator() {
    echo -e "${CYAN}────────────────────────────────────────────────────────────${NC}"
}

################################################################################
# PROCESS CHECKING
################################################################################

check_instance_status() {
    local instance=$1
    
    log_info "Checking instance: $instance"
    
    # Get instance status
    local status=$(gcloud compute instances describe $instance \
        --zone=$ZONE --project=$PROJECT_ID \
        --format="value(status)" 2>/dev/null || echo "NOT_FOUND")
    
    if [ "$status" = "RUNNING" ]; then
        log_success "Instance is RUNNING"
    else
        log_error "Instance status: $status"
        return 1
    fi
    
    # Get instance IPs
    local internal_ip=$(gcloud compute instances describe $instance \
        --zone=$ZONE --project=$PROJECT_ID \
        --format="value(networkInterfaces[0].networkIP)" 2>/dev/null)
    
    local external_ip=$(gcloud compute instances describe $instance \
        --zone=$ZONE --project=$PROJECT_ID \
        --format="value(networkInterfaces[0].accessConfigs[0].natIP)" 2>/dev/null || echo "N/A")
    
    echo "  Internal IP: $internal_ip"
    echo "  External IP: $external_ip"
    
    return 0
}

check_running_processes() {
    local instance=$1
    
    separator
    echo "Running Processes:"
    separator
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'ps aux | grep -E "customer_db|product_db|seller_server|buyer_server|buyer_client|seller_client|evaluator|cargo" | grep -v grep' \
        2>/dev/null || log_warning "No marketplace processes found"
}

check_listening_ports() {
    local instance=$1
    
    separator
    echo "Listening Ports:"
    separator
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'netstat -tlnp 2>/dev/null | grep -E ":(8080|8081|8082|8083|22)" || echo "No services listening"' \
        2>/dev/null
}

check_service_logs() {
    local instance=$1
    local service=${instance%-*}  # Remove suffix after dash
    
    separator
    echo "Recent Logs:"
    separator
    
    # Try different log locations
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        "tail -20 /var/log/marketplace/$instance.log 2>/dev/null || \
         tail -20 /var/log/$service.log 2>/dev/null || \
         tail -20 /home/appuser/online-marketplace/$service.log 2>/dev/null || \
         echo 'No logs found'" \
        2>/dev/null
}

check_disk_usage() {
    local instance=$1
    
    separator
    echo "Disk Usage:"
    separator
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'df -h | grep -E "^/dev|Filesystem"' \
        2>/dev/null
}

check_memory_usage() {
    local instance=$1
    
    separator
    echo "Memory Usage:"
    separator
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'free -h 2>/dev/null || echo "free command not available"' \
        2>/dev/null
}

check_cpu_usage() {
    local instance=$1
    
    separator
    echo "CPU Usage (top):"
    separator
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'top -bn1 2>/dev/null | head -5 || echo "top command not available"' \
        2>/dev/null
}

check_network_connectivity() {
    local instance=$1
    
    separator
    echo "Network Connectivity Test:"
    separator
    
    # Test connectivity to other services
    log_info "Testing connections to other services..."
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'echo "Testing customer-db (10.0.0.2:8080):"; nc -zv 10.0.0.2 8080 2>&1 || true;
         echo "Testing product-db (10.0.0.3:8081):"; nc -zv 10.0.0.3 8081 2>&1 || true;
         echo "Testing seller-server (10.0.0.4:8082):"; nc -zv 10.0.0.4 8082 2>&1 || true;
         echo "Testing buyer-server (10.0.0.5:8083):"; nc -zv 10.0.0.5 8083 2>&1 || true;' \
        2>/dev/null
}

check_startup_log() {
    local instance=$1
    
    separator
    echo "Startup Script Log:"
    separator
    
    gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
        'tail -50 /var/log/startup-script.log 2>/dev/null || echo "No startup log found"' \
        2>/dev/null
}

################################################################################
# COMPREHENSIVE EVALUATION
################################################################################

evaluate_single_instance() {
    local instance=$1
    
    echo ""
    log_header "Evaluating: $instance"
    
    if ! check_instance_status "$instance"; then
        log_error "Cannot evaluate - instance not running"
        return 1
    fi
    
    echo ""
    check_running_processes "$instance"
    check_listening_ports "$instance"
    check_service_logs "$instance"
    check_disk_usage "$instance"
    check_memory_usage "$instance"
    check_cpu_usage "$instance"
    check_network_connectivity "$instance"
    check_startup_log "$instance"
    
    echo ""
}

evaluate_all_instances() {
    log_header "Marketplace System Health Check"
    
    echo "Evaluating all instances..."
    echo ""
    
    local healthy=0
    local unhealthy=0
    
    for instance in "${INSTANCES[@]}"; do
        if evaluate_single_instance "$instance" 2>/dev/null; then
            ((healthy++))
        else
            ((unhealthy++))
        fi
    done
    
    # Summary
    log_header "Summary"
    echo "Instances checked: ${#INSTANCES[@]}"
    log_success "Healthy instances: $healthy"
    if [ $unhealthy -gt 0 ]; then
        log_warning "Unhealthy instances: $unhealthy"
    fi
}

generate_summary_report() {
    log_header "Summary Report"
    
    echo ""
    echo "=== INSTANCE STATUS ===" 
    gcloud compute instances list \
        --filter="name:('customer-db' OR 'product-db' OR 'seller-server' OR 'buyer-server' OR 'seller-client' OR 'buyer-client' OR 'evaluator')" \
        --format="table(name,status,INTERNAL_IP,EXTERNAL_IP,MACHINE_TYPE)" \
        --project=$PROJECT_ID
    
    echo ""
    echo "=== SERVICE STATUS ===" 
    for instance in "${INSTANCES[@]}"; do
        port=$(get_service_port "$instance")
        if [ "$port" != "none" ]; then
            echo ""
            log_info "Checking $instance (port $port)..."
            
            if gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
                "netstat -tlnp 2>/dev/null | grep -q :$port" 2>/dev/null; then
                log_success "$instance is listening on port $port"
            else
                log_warning "$instance is NOT listening on port $port"
            fi
        fi
    done
}

test_service_connectivity() {
    log_header "Service Connectivity Test"
    
    log_info "Testing buyer-server connectivity to databases..."
    
    gcloud compute ssh buyer-server --zone=$ZONE --project=$PROJECT_ID -- \
        'echo "Attempting to connect to customer-db..."; \
         (echo "PING" | nc -w 1 10.0.0.2 8080 > /dev/null 2>&1 && echo "✓ Connected to customer-db" || echo "✗ Failed to connect to customer-db"); \
         echo "Attempting to connect to product-db..."; \
         (echo "PING" | nc -w 1 10.0.0.3 8081 > /dev/null 2>&1 && echo "✓ Connected to product-db" || echo "✗ Failed to connect to product-db")' \
        2>/dev/null || log_error "Could not test connectivity"
}

################################################################################
# MAIN EXECUTION
################################################################################

main() {
    if [ $# -eq 0 ]; then
        # Evaluate all instances
        generate_summary_report
        echo ""
        evaluate_all_instances
    else
        # Evaluate specific instance
        local instance=$1
        
        if [[ " ${INSTANCES[@]} " =~ " ${instance} " ]]; then
            evaluate_single_instance "$instance"
        else
            log_error "Unknown instance: $instance"
            log_info "Available instances: ${INSTANCES[*]}"
            exit 1
        fi
    fi
    
    # Offer connectivity test
    echo ""
    read -p "Run service connectivity test? (y/n): " -r
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        test_service_connectivity
    fi
    
    log_success "Evaluation complete!"
}

################################################################################
# HELP
################################################################################

show_help() {
    cat << EOF
Process Evaluation Script for Marketplace Deployment

Usage: $0 [instance-name]

Commands:
  (no arguments)     Evaluate all instances
  instance-name      Evaluate specific instance
  --help             Show this help message

Available instances:
  - customer-db      User authentication & sessions
  - product-db       Inventory & carts
  - seller-server    Seller API frontend
  - buyer-server     Buyer API frontend
  - seller-client    Seller CLI
  - buyer-client     Buyer CLI
  - evaluator        Performance test harness

Examples:
  $0                    # Evaluate all instances
  $0 customer-db        # Evaluate customer-db only
  $0 buyer-server       # Evaluate buyer-server only

Output:
  - Process status
  - Listening ports
  - Recent logs
  - Resource usage (disk, memory, CPU)
  - Network connectivity
  - Startup logs

EOF
}

# Check for help flag
if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    show_help
    exit 0
fi

main "$@"
