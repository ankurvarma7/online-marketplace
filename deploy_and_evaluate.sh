#!/bin/bash

################################################################################
# COMPREHENSIVE GCP DEPLOYMENT & EVALUATION SCRIPT
# 
# This script deploys the online marketplace system to GCP following the
# infrastructure-as-code approach outlined in deployment.md
#
# Usage: ./deploy_and_evaluate.sh [--skip-deploy|--skip-eval|--cleanup-only]
################################################################################

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ID="online-marketplace-486000"
ZONE="us-central1-a"
REGION="us-central1"
NETWORK_NAME="marketplace-network"
SUBNET_NAME="marketplace-subnet"
SUBNET_RANGE="10.0.0.0/24"
WORKSPACE_DIR="/Users/ankurvarma/online-marketplace"

# Instance configuration (using arrays for zsh/bash compatibility)
INSTANCES_NAMES=("customer-db" "product-db" "seller-server" "buyer-server" "seller-client" "buyer-client" "evaluator")
INSTANCES_PORTS=("8080" "8081" "8082" "8083" "none" "none" "none")
INSTANCES_IPS=("10.0.0.2" "10.0.0.3" "10.0.0.4" "10.0.0.5" "10.0.0.6" "10.0.0.7" "10.0.0.8")

# GitHub repository URL - UPDATE THIS
REPO_URL="https://github.com/ankurvarma/online-marketplace.git"

################################################################################
# UTILITY FUNCTIONS
################################################################################

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v gcloud &> /dev/null; then
        log_error "gcloud CLI not found. Please install Google Cloud SDK."
        exit 1
    fi
    
    # Check if authenticated
    if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" &>/dev/null; then
        log_error "Not authenticated with gcloud. Run: gcloud auth login"
        exit 1
    fi
    
    # Set project
    gcloud config set project $PROJECT_ID 2>/dev/null || true
    
    log_success "Prerequisites check passed"
}

################################################################################
# INFRASTRUCTURE SETUP
################################################################################

create_network_infrastructure() {
    log_info "Creating network infrastructure..."
    
    # Create VPC network
    log_info "Creating VPC network: $NETWORK_NAME"
    if gcloud compute networks describe $NETWORK_NAME --project=$PROJECT_ID &>/dev/null; then
        log_warning "Network $NETWORK_NAME already exists, skipping creation"
    else
        gcloud compute networks create $NETWORK_NAME \
            --subnet-mode=custom \
            --project=$PROJECT_ID \
            --quiet
        log_success "Network created"
    fi
    
    # Create subnet
    log_info "Creating subnet: $SUBNET_NAME"
    if gcloud compute networks subnets describe $SUBNET_NAME --region=$REGION --project=$PROJECT_ID &>/dev/null; then
        log_warning "Subnet $SUBNET_NAME already exists, skipping creation"
    else
        gcloud compute networks subnets create $SUBNET_NAME \
            --network=$NETWORK_NAME \
            --range=$SUBNET_RANGE \
            --region=$REGION \
            --project=$PROJECT_ID \
            --quiet
        log_success "Subnet created"
    fi
    
    # Create firewall rules
    log_info "Creating firewall rules..."
    
    # Internal communication
    if gcloud compute firewall-rules describe marketplace-internal --project=$PROJECT_ID &>/dev/null; then
        log_warning "Firewall rule marketplace-internal already exists"
    else
        gcloud compute firewall-rules create marketplace-internal \
            --network=$NETWORK_NAME \
            --allow=tcp:8080,tcp:8081,tcp:8082,tcp:8083 \
            --source-ranges=$SUBNET_RANGE \
            --description="Allow internal communication between components" \
            --project=$PROJECT_ID \
            --quiet
        log_success "Internal firewall rule created"
    fi
    
    # SSH access
    if gcloud compute firewall-rules describe marketplace-ssh --project=$PROJECT_ID &>/dev/null; then
        log_warning "Firewall rule marketplace-ssh already exists"
    else
        gcloud compute firewall-rules create marketplace-ssh \
            --network=$NETWORK_NAME \
            --allow=tcp:22 \
            --source-ranges=0.0.0.0/0 \
            --description="Allow SSH access" \
            --project=$PROJECT_ID \
            --quiet
        log_success "SSH firewall rule created"
    fi
    
    # External access
    if gcloud compute firewall-rules describe marketplace-external --project=$PROJECT_ID &>/dev/null; then
        log_warning "Firewall rule marketplace-external already exists"
    else
        gcloud compute firewall-rules create marketplace-external \
            --network=$NETWORK_NAME \
            --allow=tcp:8082-8083 \
            --source-ranges=0.0.0.0/0 \
            --description="Allow external access to servers" \
            --project=$PROJECT_ID \
            --quiet
        log_success "External firewall rule created"
    fi
}

create_startup_script() {
    log_info "Creating startup script..."
    
    cat > /tmp/startup-script.sh << 'STARTUP_EOF'
#!/bin/bash
set -e

# Log file
LOGFILE="/var/log/startup-script.log"

{
    echo "=== Startup script started at $(date) ==="
    
    # Update system
    echo "Updating system packages..."
    apt-get update
    apt-get upgrade -y
    
    # Install dependencies
    echo "Installing dependencies..."
    apt-get install -y \
        build-essential \
        curl \
        pkg-config \
        libssl-dev \
        ca-certificates \
        vim \
        htop \
        tar
    
    # Create app user
    if ! id -u appuser &>/dev/null; then
        echo "Creating appuser..."
        useradd -m -s /bin/bash appuser
    fi
    
    # Install Rust
    echo "Installing Rust..."
    if [ ! -d /home/appuser/.cargo ]; then
        su - appuser -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
    fi
    
    # Wait for code to be transferred (will be done by main script)
    echo "Waiting for code transfer..."
    timeout=300
    elapsed=0
    while [ ! -f /home/appuser/marketplace.tar.gz ] && [ $elapsed -lt $timeout ]; do
        sleep 5
        elapsed=$((elapsed + 5))
    done
    
    if [ ! -f /home/appuser/marketplace.tar.gz ]; then
        echo "ERROR: Code transfer timeout after ${timeout}s"
        exit 1
    fi
    
    # Extract code
    echo "Extracting code..."
    su - appuser -c 'cd /home/appuser && tar xzf marketplace.tar.gz'
    
    # Verify extraction
    if [ ! -d /home/appuser/online-marketplace ]; then
        echo "ERROR: Code extraction failed"
        exit 1
    fi
    
    echo "Code extracted successfully"
    
    # Build project
    echo "Building project..."
    cd /home/appuser/online-marketplace
    su - appuser -c 'source $HOME/.cargo/env && cd /home/appuser/online-marketplace && cargo build --release 2>&1' | tail -20
    
    # Set permissions
    chown -R appuser:appuser /home/appuser/online-marketplace
    
    # Create log directory
    mkdir -p /var/log/marketplace
    chown -R appuser:appuser /var/log/marketplace
    
    echo "=== Startup script completed at $(date) ==="
} | tee -a $LOGFILE

STARTUP_EOF

    # No placeholder replacement needed anymore
    
    log_success "Startup script created at /tmp/startup-script.sh"
}

create_instances() {
    log_info "Creating VM instances..."
    
    for i in "${!INSTANCES_NAMES[@]}"; do
        instance_name="${INSTANCES_NAMES[$i]}"
        port="${INSTANCES_PORTS[$i]}"
        internal_ip="${INSTANCES_IPS[$i]}"
        
        log_info "Creating instance: $instance_name (IP: $internal_ip)"
        
        # Check if instance exists
        if gcloud compute instances describe $instance_name --zone=$ZONE --project=$PROJECT_ID &>/dev/null; then
            log_warning "Instance $instance_name already exists, skipping creation"
            continue
        fi
        
        gcloud compute instances create $instance_name \
            --zone=$ZONE \
            --machine-type=e2-micro \
            --image-family=debian-12 \
            --image-project=debian-cloud \
            --boot-disk-size=20GB \
            --boot-disk-type=pd-standard \
            --network-interface=network=$NETWORK_NAME,subnet=$SUBNET_NAME,private-network-ip=$internal_ip \
            --tags=marketplace \
            --metadata-from-file=startup-script=/tmp/startup-script.sh \
            --scopes=cloud-platform \
            --project=$PROJECT_ID \
            --quiet &
    done
    
    # Wait for all background jobs
    wait
    
    log_success "All instances created (or already exist)"
    
    # Wait for instances to be running
    log_info "Waiting for instances to reach RUNNING state..."
    for instance_name in "${INSTANCES_NAMES[@]}"; do
        while true; do
            status=$(gcloud compute instances describe $instance_name --zone=$ZONE --project=$PROJECT_ID --format="value(status)" 2>/dev/null)
            if [ "$status" = "RUNNING" ]; then
                log_success "Instance $instance_name is RUNNING"
                break
            fi
            echo -n "."
            sleep 5
        done
    done
    
    # Transfer code to all instances
    log_info "Creating tarball of local code..."
    cd "$WORKSPACE_DIR"
    tar --exclude='*/target' --exclude='.git' --exclude='.DS_Store' -czf /tmp/marketplace.tar.gz .
    
    log_info "Transferring code to all instances..."
    for instance_name in "${INSTANCES_NAMES[@]}"; do
        log_info "Transferring code to $instance_name..."
        
        # Wait for instance to be SSH-ready
        for retry in {1..5}; do
            if gcloud compute ssh "$instance_name" \
                --zone="$ZONE" \
                --project="$PROJECT_ID" \
                --command="echo 'SSH ready'" 2>/dev/null; then
                break
            fi
            log_info "Waiting for $instance_name to be SSH-ready (attempt $retry/5)..."
            sleep 10
        done
        
        # Transfer tarball
        gcloud compute scp /tmp/marketplace.tar.gz "$instance_name":/tmp/ \
            --zone="$ZONE" \
            --project="$PROJECT_ID" \
            --quiet || log_warning "Transfer to $instance_name may have failed, will retry..."
        
        # Move to appuser home and set permissions
        gcloud compute ssh "$instance_name" \
            --zone="$ZONE" \
            --project="$PROJECT_ID" \
            --command="sudo mv /tmp/marketplace.tar.gz /home/appuser/ && sudo chown appuser:appuser /home/appuser/marketplace.tar.gz" \
            || log_warning "Permission setup for $instance_name may have failed"
        
        log_success "Code transferred to $instance_name"
    done
    
    log_success "Code transferred to all instances!"
    rm -f /tmp/marketplace.tar.gz
    
    # Wait for builds to complete
    log_info "Waiting for builds to complete (this may take 5-10 minutes)..."
    sleep 300
}

wait_for_services() {
    log_info "Waiting for services to start on instances..."
    
    # Function to check if service is running
    check_service() {
        local instance=$1
        local service=$2
        local port=$3
        
        for i in {1..60}; do
            if gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
                "netstat -tlnp 2>/dev/null | grep -q :$port" 2>/dev/null; then
                log_success "Service $service is listening on port $port"
                return 0
            fi
            echo -n "."
            sleep 5
        done
        return 1
    }
    
    # Check backend services
    check_service "customer-db" "customer-db" "8080" || log_warning "customer-db not responding"
    check_service "product-db" "product-db" "8081" || log_warning "product-db not responding"
    check_service "seller-server" "seller-server" "8082" || log_warning "seller-server not responding"
    check_service "buyer-server" "buyer-server" "8083" || log_warning "buyer-server not responding"
    
    log_success "Service startup check complete"
}

configure_services() {
    log_info "Configuring services on instances..."
    
    # Function to configure and start a service
    configure_service() {
        local instance=$1
        local service_name=$2
        local port=$3
        local env_vars=$4
        
        log_info "Configuring $service_name on $instance..."
        
        gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
            "mkdir -p /etc/marketplace && echo '$env_vars' > /etc/marketplace/$service_name.env" \
            2>/dev/null || true
    }
    
    # Customer DB
    configure_service "customer-db" "customer-db" "8080" \
        "CUSTOMER_DB_BIND_ADDR=10.0.0.2:8080"
    
    # Product DB
    configure_service "product-db" "product-db" "8081" \
        "PRODUCT_DB_BIND_ADDR=10.0.0.3:8081"
    
    # Seller Server
    configure_service "seller-server" "seller-server" "8082" \
        "SELLER_SERVER_BIND_ADDR=10.0.0.4:8082
CUSTOMER_DB_ADDR=10.0.0.2:8080
PRODUCT_DB_ADDR=10.0.0.3:8081"
    
    # Buyer Server
    configure_service "buyer-server" "buyer-server" "8083" \
        "BUYER_SERVER_BIND_ADDR=10.0.0.5:8083
CUSTOMER_DB_ADDR=10.0.0.2:8080
PRODUCT_DB_ADDR=10.0.0.3:8081"
    
    log_success "Service configuration complete"
}

################################################################################
# START SERVICES
################################################################################

start_services() {
    log_info "Starting services on instances..."
    
    # Function to start a service
    start_service() {
        local instance=$1
        local service_name=$2
        local main_name=${service_name%-*}  # Remove suffix after dash
        
        log_info "Starting $service_name on $instance..."
        
        gcloud compute ssh $instance --zone=$ZONE --project=$PROJECT_ID -- \
            "cd /home/appuser/online-marketplace && \
             source /home/appuser/.cargo/env && \
             source /etc/marketplace/$service_name.env 2>/dev/null || true && \
             nohup ./target/release/$main_name > /var/log/marketplace/$service_name.log 2>&1 &" \
            2>/dev/null || log_warning "Failed to start $service_name"
    }
    
    # Start backend services first
    start_service "customer-db" "customer-db"
    sleep 5
    
    start_service "product-db" "product-db"
    sleep 5
    
    # Then frontend services
    start_service "seller-server" "seller-server"
    sleep 5
    
    start_service "buyer-server" "buyer-server"
    sleep 5
    
    log_success "Service startup commands sent"
}

################################################################################
# EVALUATION
################################################################################

run_evaluation() {
    log_info "Running performance evaluation..."
    
    # Wait a bit for services to stabilize
    log_info "Waiting for services to stabilize (30 seconds)..."
    sleep 30
    
    # Get external IP of evaluator
    evaluator_ip=$(gcloud compute instances describe evaluator \
        --zone=$ZONE --project=$PROJECT_ID \
        --format="value(networkInterfaces[0].accessConfigs[0].natIP)" 2>/dev/null)
    
    if [ -z "$evaluator_ip" ]; then
        log_warning "Could not get evaluator external IP, using internal IP for SSH"
        evaluator_ip="evaluator"
    fi
    
    log_info "Running evaluation on evaluator instance..."
    
    gcloud compute ssh evaluator --zone=$ZONE --project=$PROJECT_ID -- \
        "cd /home/appuser/online-marketplace && \
         source /home/appuser/.cargo/env && \
         export SELLER_SERVER_ADDR=10.0.0.4:8082 && \
         export BUYER_SERVER_ADDR=10.0.0.5:8083 && \
         timeout 600 ./target/release/evaluator 2>&1 | tee /var/log/marketplace/evaluation.log" \
        2>/dev/null
    
    # Copy evaluation results
    log_info "Copying evaluation results..."
    timestamp=$(date +%Y%m%d_%H%M%S)
    
    gcloud compute scp evaluator:/var/log/marketplace/evaluation.log \
        ./gcp_evaluation_$timestamp.txt \
        --zone=$ZONE --project=$PROJECT_ID \
        2>/dev/null || log_warning "Could not copy evaluation results"
    
    log_success "Evaluation complete - Results saved to gcp_evaluation_$timestamp.txt"
}

################################################################################
# EVALUATION DETAILS (Process Status)
################################################################################

evaluate_processes() {
    log_info "Evaluating running processes on instances..."
    
    echo ""
    echo "=== PROCESS STATUS REPORT ===" > process_status_report.txt
    echo "Generated: $(date)" >> process_status_report.txt
    echo "" >> process_status_report.txt
    
    for instance_name in "${!INSTANCES[@]}"; do
        echo "" >> process_status_report.txt
        echo "--- $instance_name ---" >> process_status_report.txt
        
        gcloud compute ssh $instance_name --zone=$ZONE --project=$PROJECT_ID -- \
            "echo 'Running processes:'; ps aux | grep -E 'customer_db|product_db|seller_server|buyer_server|evaluator' | grep -v grep; \
             echo ''; \
             echo 'Network listening ports:'; netstat -tlnp 2>/dev/null | grep -E ':(8080|8081|8082|8083)' || echo 'None found'" \
            2>/dev/null >> process_status_report.txt || echo "Failed to get status from $instance_name" >> process_status_report.txt
    done
    
    log_success "Process status report saved to process_status_report.txt"
    cat process_status_report.txt
}

get_instance_info() {
    log_info "Getting instance information..."
    
    echo ""
    echo "=== INSTANCE INFORMATION ===" 
    gcloud compute instances list \
        --filter="name:('customer-db' OR 'product-db' OR 'seller-server' OR 'buyer-server' OR 'seller-client' OR 'buyer-client' OR 'evaluator')" \
        --format="table(name,status,INTERNAL_IP,EXTERNAL_IP)" \
        --project=$PROJECT_ID
    echo ""
}

################################################################################
# CLEANUP
################################################################################

cleanup_resources() {
    log_warning "Cleaning up GCP resources..."
    
    read -p "Are you sure you want to delete all marketplace resources? (yes/no): " -r
    if [[ $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
        log_info "Deleting instances..."
        for instance_name in "${INSTANCES_NAMES[@]}"; do
            gcloud compute instances delete $instance_name \
                --zone=$ZONE --quiet --project=$PROJECT_ID 2>/dev/null &
        done
        wait
        
        log_info "Deleting firewall rules..."
        gcloud compute firewall-rules delete marketplace-internal \
            --quiet --project=$PROJECT_ID 2>/dev/null &
        gcloud compute firewall-rules delete marketplace-ssh \
            --quiet --project=$PROJECT_ID 2>/dev/null &
        gcloud compute firewall-rules delete marketplace-external \
            --quiet --project=$PROJECT_ID 2>/dev/null &
        wait
        
        log_info "Deleting subnet..."
        gcloud compute networks subnets delete $SUBNET_NAME \
            --region=$REGION --quiet --project=$PROJECT_ID 2>/dev/null
        
        log_info "Deleting network..."
        gcloud compute networks delete $NETWORK_NAME \
            --quiet --project=$PROJECT_ID 2>/dev/null
        
        log_success "Cleanup complete"
    else
        log_warning "Cleanup cancelled"
    fi
}

################################################################################
# MAIN EXECUTION
################################################################################

main() {
    local skip_deploy=false
    local skip_eval=false
    local cleanup_only=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --skip-deploy) skip_deploy=true; shift ;;
            --skip-eval) skip_eval=true; shift ;;
            --cleanup-only) cleanup_only=true; shift ;;
            *) echo "Unknown option: $1"; exit 1 ;;
        esac
    done
    
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║  GCP Marketplace Deployment & Evaluation Script               ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    echo ""
    
    if [ "$cleanup_only" = true ]; then
        cleanup_resources
        exit 0
    fi
    
    check_prerequisites
    
    if [ "$skip_deploy" = false ]; then
        log_info "DEPLOYMENT PHASE"
        echo "================================================================"
        
        create_network_infrastructure
        create_startup_script
        create_instances
        
        # Wait for builds to complete (can take 5-10 minutes)
        log_info "Waiting for instances to build (5-10 minutes)..."
        sleep 300  # 5 minutes
        
        get_instance_info
        wait_for_services
        configure_services
        start_services
        
        log_success "DEPLOYMENT COMPLETE"
    fi
    
    echo ""
    echo "================================================================"
    
    if [ "$skip_eval" = false ]; then
        log_info "EVALUATION PHASE"
        echo "================================================================"
        
        # Wait a bit more for services to fully start
        sleep 30
        
        evaluate_processes
        echo ""
        
        run_evaluation
        
        log_success "EVALUATION COMPLETE"
    fi
    
    echo ""
    echo "================================================================"
    log_success "All operations complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Review results: cat gcp_evaluation_*.txt"
    echo "  2. Check process status: cat process_status_report.txt"
    echo "  3. SSH to instance: gcloud compute ssh customer-db --zone=$ZONE"
    echo "  4. View logs: gcloud compute ssh SERVICE-NAME --zone=$ZONE -- 'tail /var/log/marketplace/SERVICE-NAME.log'"
    echo "  5. Cleanup: $0 --cleanup-only"
    echo ""
}

main "$@"
