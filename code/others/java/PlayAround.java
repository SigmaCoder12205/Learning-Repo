import java.util.Scanner;
public class PlayAround{
    private static final Scanner scanner = new Scanner(System.in);
    public static void main(String[] args){
        System.out.print("Enter your currencey: ");
        String currencey = scanner.nextLine();
        System.out.print("Enter your amount: ");
        double amount = scanner.nextDouble();
        double finalAmount = amount * 1.42;
        System.out.print("USD to AUD Final Amount: " + finalAmount);
        scanner.close();
    }
}