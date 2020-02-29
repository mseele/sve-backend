package de.sve.backend.mail;

import java.util.Properties;

import javax.mail.Address;
import javax.mail.Authenticator;
import javax.mail.Message;
import javax.mail.MessagingException;
import javax.mail.PasswordAuthentication;
import javax.mail.Session;
import javax.mail.Transport;
import javax.mail.internet.InternetAddress;
import javax.mail.internet.MimeMessage;

/**
 * Resposible for sending emails.
 * 
 * @author mseele
 */
public class Postman {

	protected static void deliver(Mail mail) throws MessagingException {
		MailAccount account = mail.sender();
		// smtp properties
		Properties properties = System.getProperties();
		properties.put("mail.smtp.host", "smtp.gmail.com"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.port", "465"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.ssl.enable", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.auth", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		// authenticate
		Session session = Session.getInstance(properties, new Authenticator() {
			@Override
			protected PasswordAuthentication getPasswordAuthentication() {
				return new PasswordAuthentication(account.userName(), account.password());
			}
		});
		MimeMessage message = new MimeMessage(session);
		// email addresses
		message.setFrom(new InternetAddress(account.email()));
		for (String recipient : mail.to()) {
			message.addRecipient(Message.RecipientType.TO, new InternetAddress(recipient));
		}
		for (String recipient : mail.bcc()) {
			message.addRecipient(Message.RecipientType.BCC, new InternetAddress(recipient));
		}
		String replyTo = mail.replyTo();
		if (replyTo != null) {
			message.setReplyTo(new Address[] { new InternetAddress(replyTo) });
		}
		// subject & content
		message.setSubject(mail.subject());
		message.setText(mail.content());
		// Send message
		Transport.send(message);
	}

}
