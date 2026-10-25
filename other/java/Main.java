

import java.util.ArrayList;
import java.util.Random;
import java.util.Scanner;

public class Main{
    private static final Scanner scanner = new Scanner(System.in);
    private static final ArrayList<String> answers = new ArrayList<>(); 
    private static final Random random = new Random();
    public static void main(String[] args) {
        addAnswers();
        final String computerChoice = generate();
        System.out.println("Rock Paper Scissors!");
        System.out.print("Rock Paper or Scissors?: ");
        String guess = scanner.nextLine().toLowerCase();

        if (answers.contains(guess)) {
            if (guess.equals(computerChoice)) { 
                System.out.println("Computer choice: " + computerChoice);
                System.out.println("It's a tie");
            }
            else if (guess.equals(answers.get(0)) && computerChoice.equals(answers.get(2))
                     || guess.equals(answers.get(1)) && computerChoice.equals(answers.get(0))
                     || guess.equals(answers.get(2)) && computerChoice.equals(answers.get(1))){
                        System.out.println("Computer Choice: " + computerChoice);
                        System.out.println("You win!");
            }
            else{
                System.out.println("Computer choice: " + computerChoice);
                System.out.println("You lose");
            }
            
        }
    }

    private static void addAnswers(){
        answers.add("rock");
        answers.add("paper");
        answers.add("scissors");
    }

    private static String generate(){
        final int randomIndex = random.nextInt(answers.size());
        final String computerChoice = answers.get(randomIndex);

        return computerChoice;
    }
}